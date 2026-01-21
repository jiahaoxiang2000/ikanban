use axum::{
    Router,
    extract::{State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{
    AppState,
    entities::{
        execution_process, execution_process_logs, project,
        response::{SubscribeTarget, WsEvent, WsMessage, WsRequest, WsResponse, WsResponseData},
        session, task,
    },
};

/// GET /api/ws - Main WebSocket endpoint for all operations
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_ws_connection(socket, state).await {
            tracing::warn!("WebSocket connection closed: {}", e);
        }
    })
}

async fn handle_ws_connection(
    socket: axum::extract::ws::WebSocket,
    state: AppState,
) -> anyhow::Result<()> {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = state.subscribe();

    // Track subscriptions for this connection
    let subscriptions: Arc<Mutex<Vec<SubscribeTarget>>> = Arc::new(Mutex::new(Vec::new()));

    // Send initial connected message
    let connected_msg = serde_json::to_string(&WsMessage::Connected)?;
    sender
        .send(axum::extract::ws::Message::Text(connected_msg.into()))
        .await?;

    // Spawn task to handle incoming messages (requests)
    let sender_clone = Arc::new(Mutex::new(sender));
    let sender_for_recv = sender_clone.clone();
    let state_clone = state.clone();
    let subscriptions_clone = subscriptions.clone();

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let axum::extract::ws::Message::Text(text) = msg {
                if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                    match ws_msg {
                        WsMessage::Request { id, request } => {
                            let response =
                                handle_request(request, &state_clone, &subscriptions_clone).await;
                            let response_msg = WsMessage::Response { id, response };

                            if let Ok(json) = serde_json::to_string(&response_msg) {
                                let mut sender = sender_for_recv.lock().await;
                                let _ = sender
                                    .send(axum::extract::ws::Message::Text(json.into()))
                                    .await;
                            }
                        }
                        WsMessage::Ping => {
                            let pong_msg = serde_json::to_string(&WsMessage::Pong).unwrap();
                            let mut sender = sender_for_recv.lock().await;
                            let _ = sender
                                .send(axum::extract::ws::Message::Text(pong_msg.into()))
                                .await;
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    // Forward events to client based on subscriptions
    loop {
        match event_rx.recv().await {
            Ok(event) => {
                let subs = subscriptions.lock().await;
                if should_send_event(&event, &subs) {
                    let msg = WsMessage::Event(event);
                    if let Ok(json) = serde_json::to_string(&msg) {
                        let mut sender = sender_clone.lock().await;
                        if sender
                            .send(axum::extract::ws::Message::Text(json.into()))
                            .await
                            .is_err()
                        {
                            break; // Client disconnected
                        }
                    }
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("WebSocket client lagged by {} messages", n);
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                break;
            }
        }
    }

    recv_task.abort();
    Ok(())
}

async fn handle_request(
    request: WsRequest,
    state: &AppState,
    subscriptions: &Arc<Mutex<Vec<SubscribeTarget>>>,
) -> WsResponse {
    match request {
        // Projects
        WsRequest::ListProjects => match project::Model::find_all_with_status(&state.db).await {
            Ok(projects) => WsResponse::Success(WsResponseData::Projects(projects)),
            Err(e) => WsResponse::Error {
                message: e.to_string(),
            },
        },
        WsRequest::GetProject { id } => match project::Model::find_by_id(&state.db, id).await {
            Ok(Some(project)) => WsResponse::Success(WsResponseData::Project(project)),
            Ok(None) => WsResponse::Error {
                message: format!("Project {} not found", id),
            },
            Err(e) => WsResponse::Error {
                message: e.to_string(),
            },
        },
        WsRequest::CreateProject(payload) => {
            if payload.name.trim().is_empty() {
                return WsResponse::Error {
                    message: "Project name cannot be empty".to_string(),
                };
            }

            match project::Model::create(&state.db, &payload).await {
                Ok(project) => {
                    state.broadcast(WsEvent::ProjectCreated(project.clone()));
                    WsResponse::Success(WsResponseData::Project(project))
                }
                Err(e) => WsResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        WsRequest::UpdateProject { id, data } => {
            match project::Model::update(&state.db, id, &data).await {
                Ok(Some(project)) => {
                    state.broadcast(WsEvent::ProjectUpdated(project.clone()));
                    WsResponse::Success(WsResponseData::Project(project))
                }
                Ok(None) => WsResponse::Error {
                    message: format!("Project {} not found", id),
                },
                Err(e) => WsResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        WsRequest::DeleteProject { id } => {
            // Delete all tasks first
            match task::Model::delete_by_project_id(&state.db, id).await {
                Ok(_) => {}
                Err(e) => {
                    return WsResponse::Error {
                        message: e.to_string(),
                    };
                }
            }

            match project::Model::delete(&state.db, id).await {
                Ok(true) => {
                    state.broadcast(WsEvent::ProjectDeleted { id });
                    WsResponse::Success(WsResponseData::Empty)
                }
                Ok(false) => WsResponse::Error {
                    message: format!("Project {} not found", id),
                },
                Err(e) => WsResponse::Error {
                    message: e.to_string(),
                },
            }
        }

        // Tasks
        WsRequest::ListTasks { project_id } => {
            match task::Model::find_by_project_id(&state.db, project_id).await {
                Ok(tasks) => {
                    // Convert to TaskWithSessionStatus (for now, just wrap with default values)
                    let tasks_with_status: Vec<task::TaskWithSessionStatus> = tasks
                        .into_iter()
                        .map(|task| task::TaskWithSessionStatus {
                            task,
                            session_count: 0,
                            has_running_session: false,
                            last_session_failed: false,
                        })
                        .collect();
                    WsResponse::Success(WsResponseData::Tasks(tasks_with_status))
                }
                Err(e) => WsResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        WsRequest::GetTask { id } => match task::Model::find_by_id(&state.db, id).await {
            Ok(Some(task)) => WsResponse::Success(WsResponseData::Task(task)),
            Ok(None) => WsResponse::Error {
                message: format!("Task {} not found", id),
            },
            Err(e) => WsResponse::Error {
                message: e.to_string(),
            },
        },
        WsRequest::CreateTask(payload) => match task::Model::create(&state.db, &payload).await {
            Ok(task) => {
                state.broadcast(WsEvent::TaskCreated(task.clone()));
                WsResponse::Success(WsResponseData::Task(task))
            }
            Err(e) => WsResponse::Error {
                message: e.to_string(),
            },
        },
        WsRequest::UpdateTask { id, data } => {
            match task::Model::update(&state.db, id, &data).await {
                Ok(Some(task)) => {
                    state.broadcast(WsEvent::TaskUpdated(task.clone()));
                    WsResponse::Success(WsResponseData::Task(task))
                }
                Ok(None) => WsResponse::Error {
                    message: format!("Task {} not found", id),
                },
                Err(e) => WsResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        WsRequest::DeleteTask { id } => match task::Model::delete(&state.db, id).await {
            Ok(true) => {
                state.broadcast(WsEvent::TaskDeleted { id });
                WsResponse::Success(WsResponseData::Empty)
            }
            Ok(false) => WsResponse::Error {
                message: format!("Task {} not found", id),
            },
            Err(e) => WsResponse::Error {
                message: e.to_string(),
            },
        },

        // Sessions
        WsRequest::ListSessions { task_id } => {
            match session::Model::find_by_task_id(&state.db, task_id).await {
                Ok(sessions) => WsResponse::Success(WsResponseData::Sessions(sessions)),
                Err(e) => WsResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        WsRequest::GetSession { task_id: _, id } => {
            match session::Model::find_by_id(&state.db, id).await {
                Ok(Some(session)) => WsResponse::Success(WsResponseData::Session(session)),
                Ok(None) => WsResponse::Error {
                    message: format!("Session {} not found", id),
                },
                Err(e) => WsResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        WsRequest::CreateSession(payload) => {
            match session::Model::create(&state.db, &payload).await {
                Ok(session) => {
                    state.broadcast(WsEvent::SessionCreated(session.clone()));
                    WsResponse::Success(WsResponseData::Session(session))
                }
                Err(e) => WsResponse::Error {
                    message: e.to_string(),
                },
            }
        }

        // Executions
        WsRequest::ListExecutions { session_id } => {
            match execution_process::Model::find_by_session_id(&state.db, session_id).await {
                Ok(executions) => WsResponse::Success(WsResponseData::Executions(executions)),
                Err(e) => WsResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        WsRequest::GetExecution { id } => {
            match execution_process::Model::find_by_id(&state.db, id).await {
                Ok(Some(execution)) => WsResponse::Success(WsResponseData::Execution(execution)),
                Ok(None) => WsResponse::Error {
                    message: format!("Execution {} not found", id),
                },
                Err(e) => WsResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        WsRequest::CreateExecution(payload) => {
            match execution_process::Model::create(&state.db, &payload).await {
                Ok(execution) => {
                    state.broadcast(WsEvent::ExecutionCreated(execution.clone()));
                    WsResponse::Success(WsResponseData::Execution(execution))
                }
                Err(e) => WsResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        WsRequest::StopExecution { id } => {
            match execution_process::Model::stop(&state.db, id).await {
                Ok(Some(execution)) => {
                    state.broadcast(WsEvent::ExecutionUpdated(execution.clone()));
                    WsResponse::Success(WsResponseData::Execution(execution))
                }
                Ok(None) => WsResponse::Error {
                    message: format!("Execution {} not found", id),
                },
                Err(e) => WsResponse::Error {
                    message: e.to_string(),
                },
            }
        }

        // Execution Logs
        WsRequest::GetExecutionLogs {
            execution_id,
            limit,
        } => {
            let result = if let Some(limit) = limit {
                execution_process_logs::Model::find_recent_by_execution_process_id(
                    &state.db,
                    execution_id,
                    limit,
                )
                .await
            } else {
                execution_process_logs::Model::find_by_execution_process_id(&state.db, execution_id)
                    .await
            };

            match result {
                Ok(logs) => WsResponse::Success(WsResponseData::ExecutionLogs(logs)),
                Err(e) => WsResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        WsRequest::CreateExecutionLog(payload) => {
            match execution_process_logs::Model::create(&state.db, &payload).await {
                Ok(log) => {
                    // Broadcast log event
                    state.broadcast(WsEvent::Log {
                        execution_id: log.execution_process_id,
                        content: log.message.clone(),
                    });
                    WsResponse::Success(WsResponseData::Empty)
                }
                Err(e) => WsResponse::Error {
                    message: e.to_string(),
                },
            }
        }

        // Subscriptions
        WsRequest::Subscribe(target) => {
            let mut subs = subscriptions.lock().await;
            if !subs.iter().any(|s| matches_target(s, &target)) {
                subs.push(target);
            }
            WsResponse::Success(WsResponseData::Empty)
        }
        WsRequest::Unsubscribe(target) => {
            let mut subs = subscriptions.lock().await;
            subs.retain(|s| !matches_target(s, &target));
            WsResponse::Success(WsResponseData::Empty)
        }
    }
}

fn matches_target(a: &SubscribeTarget, b: &SubscribeTarget) -> bool {
    match (a, b) {
        (SubscribeTarget::Projects, SubscribeTarget::Projects) => true,
        (
            SubscribeTarget::Tasks { project_id: a_id },
            SubscribeTarget::Tasks { project_id: b_id },
        ) => a_id == b_id,
        (
            SubscribeTarget::ExecutionLogs { execution_id: a_id },
            SubscribeTarget::ExecutionLogs { execution_id: b_id },
        ) => a_id == b_id,
        _ => false,
    }
}

fn should_send_event(event: &WsEvent, subscriptions: &[SubscribeTarget]) -> bool {
    for sub in subscriptions {
        match (sub, event) {
            (SubscribeTarget::Projects, WsEvent::ProjectCreated(_))
            | (SubscribeTarget::Projects, WsEvent::ProjectUpdated(_))
            | (SubscribeTarget::Projects, WsEvent::ProjectDeleted { .. }) => return true,

            (
                SubscribeTarget::Tasks { project_id },
                WsEvent::TaskCreated(task) | WsEvent::TaskUpdated(task),
            ) if &task.project_id == project_id => return true,

            (SubscribeTarget::Tasks { project_id: _ }, WsEvent::TaskDeleted { .. }) => {
                // For deleted tasks, we can't check project_id, so send to all task subscribers
                return true;
            }

            (
                SubscribeTarget::ExecutionLogs { execution_id },
                WsEvent::Log {
                    execution_id: log_exec_id,
                    ..
                },
            ) if execution_id == log_exec_id => return true,

            _ => {}
        }
    }
    false
}

pub fn router() -> Router<AppState> {
    Router::new().route("/ws", get(ws_handler))
}
