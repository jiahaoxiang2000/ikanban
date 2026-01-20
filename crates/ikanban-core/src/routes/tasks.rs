use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{ApiResponse, CreateTask, Project, Task, TaskQuery, UpdateTask, WsEvent},
    AppState,
};

/// GET /api/tasks?project_id={id} - List tasks for a project
pub async fn list_tasks(
    State(state): State<AppState>,
    Query(query): Query<TaskQuery>,
) -> Result<Json<ApiResponse<Vec<Task>>>, AppError> {
    // Verify project exists
    let _ = Project::find_by_id(&state.db, query.project_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project {} not found", query.project_id)))?;

    let tasks = Task::find_by_project_id(&state.db, query.project_id).await?;
    Ok(Json(ApiResponse::success(tasks)))
}

/// POST /api/tasks - Create a new task
pub async fn create_task(
    State(state): State<AppState>,
    Json(payload): Json<CreateTask>,
) -> Result<Json<ApiResponse<Task>>, AppError> {
    if payload.title.trim().is_empty() {
        return Err(AppError::BadRequest("Task title cannot be empty".to_string()));
    }

    // Verify project exists
    let _ = Project::find_by_id(&state.db, payload.project_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project {} not found", payload.project_id)))?;

    let task = Task::create(&state.db, &payload).await?;

    // Broadcast event
    state.broadcast(WsEvent::TaskCreated(task.clone()));

    tracing::info!(
        "Created task: {} ({}) in project {}",
        task.title,
        task.id,
        task.project_id
    );
    Ok(Json(ApiResponse::success(task)))
}

/// GET /api/tasks/{id} - Get task by ID
pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Task>>, AppError> {
    let task = Task::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Task {} not found", id)))?;

    Ok(Json(ApiResponse::success(task)))
}

/// PUT /api/tasks/{id} - Update task
pub async fn update_task(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTask>,
) -> Result<Json<ApiResponse<Task>>, AppError> {
    let task = Task::update(&state.db, id, &payload)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Task {} not found", id)))?;

    // Broadcast event
    state.broadcast(WsEvent::TaskUpdated(task.clone()));

    tracing::info!("Updated task: {} ({})", task.title, task.id);
    Ok(Json(ApiResponse::success(task)))
}

/// DELETE /api/tasks/{id} - Delete task
pub async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let deleted = Task::delete(&state.db, id).await?;

    if !deleted {
        return Err(AppError::NotFound(format!("Task {} not found", id)));
    }

    // Broadcast event
    state.broadcast(WsEvent::TaskDeleted { id });

    tracing::info!("Deleted task: {}", id);
    Ok(Json(ApiResponse::success(())))
}

/// GET /api/tasks/stream/ws?project_id={id} - WebSocket stream for task updates
pub async fn stream_tasks_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<TaskQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_tasks_ws(socket, state, query.project_id).await {
            tracing::warn!("Tasks WebSocket closed: {}", e);
        }
    })
}

async fn handle_tasks_ws(
    socket: axum::extract::ws::WebSocket,
    state: AppState,
    project_id: Uuid,
) -> anyhow::Result<()> {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = state.subscribe();

    // Send initial connected message
    let connected_msg = serde_json::to_string(&WsEvent::Connected)?;
    sender
        .send(axum::extract::ws::Message::Text(connected_msg.into()))
        .await?;

    // Spawn task to handle incoming messages (for ping/pong)
    tokio::spawn(async move {
        while let Some(Ok(_msg)) = receiver.next().await {
            // Handle incoming messages if needed
        }
    });

    // Forward task events to client (filtered by project_id)
    loop {
        match event_rx.recv().await {
            Ok(event) => {
                // Only forward task events for the specified project
                let should_send = match &event {
                    WsEvent::TaskCreated(task) => task.project_id == project_id,
                    WsEvent::TaskUpdated(task) => task.project_id == project_id,
                    WsEvent::TaskDeleted { .. } => true, // Can't filter by project_id here
                    _ => false,
                };

                if should_send {
                    let msg = serde_json::to_string(&event)?;
                    if sender
                        .send(axum::extract::ws::Message::Text(msg.into()))
                        .await
                        .is_err()
                    {
                        break; // Client disconnected
                    }
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("WebSocket client lagged by {} messages", n);
            }
            Err(broadcast::error::RecvError::Closed) => {
                break;
            }
        }
    }

    Ok(())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tasks", get(list_tasks).post(create_task))
        .route("/tasks/stream/ws", get(stream_tasks_ws))
        .route(
            "/tasks/{id}",
            get(get_task).put(update_task).delete(delete_task),
        )
}
