use axum::{
    Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::{Json, Response},
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    AppState,
    entities::{
        execution_process::{CreateExecutionProcess, Model as ExecutionProcess},
        execution_process_logs::{CreateExecutionProcessLog, Model as ExecutionProcessLog},
        response::{ApiResponse, WsEvent},
        session::Model as Session,
    },
    error::AppError,
};

/// GET /api/sessions/{session_id}/executions - List executions for a session
pub async fn list_executions(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<ExecutionProcess>>>, AppError> {
    // Verify session exists
    let _ = Session::find_by_id(&state.db, session_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Session {} not found", session_id)))?;

    let executions = ExecutionProcess::find_by_session_id(&state.db, session_id).await?;
    Ok(Json(ApiResponse::success(executions)))
}

/// POST /api/sessions/{session_id}/executions - Create a new execution
pub async fn create_execution(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(mut payload): Json<CreateExecutionProcess>,
) -> Result<Json<ApiResponse<ExecutionProcess>>, AppError> {
    // Ensure payload session_id matches path session_id
    payload.session_id = session_id;

    // Verify session exists
    let _ = Session::find_by_id(&state.db, session_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Session {} not found", session_id)))?;

    let execution = ExecutionProcess::create(&state.db, &payload).await?;

    // Broadcast event
    state.broadcast(WsEvent::ExecutionCreated(execution.clone()));

    tracing::info!(
        "Created execution: {} for session {}",
        execution.id,
        execution.session_id
    );
    Ok(Json(ApiResponse::success(execution)))
}

/// GET /api/executions/{id} - Get execution by ID
pub async fn get_execution(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<ExecutionProcess>>, AppError> {
    let execution = ExecutionProcess::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Execution {} not found", id)))?;

    Ok(Json(ApiResponse::success(execution)))
}

/// POST /api/executions/{id}/stop - Stop an execution
pub async fn stop_execution(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<ExecutionProcess>>, AppError> {
    let execution = ExecutionProcess::stop(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Execution {} not found", id)))?;

    // Broadcast event
    state.broadcast(WsEvent::ExecutionUpdated(execution.clone()));

    tracing::info!("Stopped execution: {}", id);
    Ok(Json(ApiResponse::success(execution)))
}

pub async fn logs_stream(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state, id))
}

async fn handle_socket(mut socket: WebSocket, state: AppState, execution_id: Uuid) {
    let mut rx = state.subscribe();

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(WsEvent::Log { execution_id: eid, content }) if eid == execution_id => {
                        if socket.send(Message::Text(content.into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {} // Ignore other events
                    Err(_) => {} // Lagged
                }
            }
            // Check for client disconnect
            client_msg = socket.recv() => {
                match client_msg {
                    Some(Ok(_)) => {}, // Ignore client messages
                    Some(Err(_)) | None => break, // Client disconnected
                }
            }
        }
    }
}

/// GET /api/executions/{id}/logs - Get logs for an execution
pub async fn get_logs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(params): Query<Option<std::collections::HashMap<String, String>>>,
) -> Result<Json<ApiResponse<Vec<ExecutionProcessLog>>>, AppError> {
    // Verify execution exists
    let _ = ExecutionProcess::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Execution {} not found", id)))?;

    // Get optional limit parameter (clone to avoid borrow issues)
    let params = params.as_ref();
    let limit = params
        .and_then(|p| p.get("limit"))
        .and_then(|l| l.parse().ok())
        .unwrap_or(100);

    let logs = ExecutionProcessLog::find_recent_by_execution_process_id(&state.db, id, limit).await?;
    Ok(Json(ApiResponse::success(logs)))
}

/// POST /api/executions/{id}/logs - Create a log entry for an execution
pub async fn create_log(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<CreateExecutionProcessLog>,
) -> Result<Json<ApiResponse<ExecutionProcessLog>>, AppError> {
    // Verify execution exists
    let _ = ExecutionProcess::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Execution {} not found", id)))?;

    // Ensure payload execution_process_id matches path id
    let payload = CreateExecutionProcessLog {
        execution_process_id: id,
        level: payload.level,
        message: payload.message,
    };

    let log = ExecutionProcessLog::create(&state.db, &payload).await?;

    // Broadcast log event for real-time streaming
    state.broadcast(WsEvent::Log {
        execution_id: id,
        content: log.message.clone(),
    });

    tracing::debug!("Created log entry for execution: {}", id);
    Ok(Json(ApiResponse::success(log)))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/sessions/{session_id}/executions",
            get(list_executions).post(create_execution),
        )
        .route("/executions/{id}", get(get_execution))
        .route("/executions/{id}/stop", post(stop_execution))
        .route("/executions/{id}/logs", get(get_logs).post(create_log))
        .route("/executions/{id}/logs/stream", get(logs_stream))
}
