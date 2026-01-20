use axum::{
    extract::{Path, State},
    response::Json,
    routing::get,
    Router,
};
use uuid::Uuid;

use crate::{
    error::AppError,
    entities::{
        response::{ApiResponse, WsEvent},
        session::{CreateSession, Model as Session},
        task::{Model as Task},
    },
    AppState,
};

/// GET /api/tasks/{task_id}/sessions - List sessions for a task
pub async fn list_sessions(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<Session>>>, AppError> {
    // Verify task exists
    let _ = Task::find_by_id(&state.db, task_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Task {} not found", task_id)))?;

    let sessions = Session::find_by_task_id(&state.db, task_id).await?;
    Ok(Json(ApiResponse::success(sessions)))
}

/// POST /api/tasks/{task_id}/sessions - Create a new session
pub async fn create_session(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
    Json(mut payload): Json<CreateSession>,
) -> Result<Json<ApiResponse<Session>>, AppError> {
    // Ensure payload task_id matches path task_id
    payload.task_id = task_id;

    // Verify task exists
    let _ = Task::find_by_id(&state.db, task_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Task {} not found", task_id)))?;

    let session = Session::create(&state.db, &payload).await?;

    // Broadcast event
    state.broadcast(WsEvent::SessionCreated(session.clone()));

    tracing::info!(
        "Created session: {} for task {}",
        session.id,
        session.task_id
    );
    Ok(Json(ApiResponse::success(session)))
}

/// GET /api/tasks/{task_id}/sessions/{id} - Get session by ID
pub async fn get_session(
    State(state): State<AppState>,
    Path((task_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<Session>>, AppError> {
    // Verify task exists (optional but good for consistency)
    let _ = Task::find_by_id(&state.db, task_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Task {} not found", task_id)))?;

    let session = Session::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Session {} not found", id)))?;

    // Verify session belongs to task
    if session.task_id != task_id {
        return Err(AppError::BadRequest(format!(
            "Session {} does not belong to task {}",
            id, task_id
        )));
    }

    Ok(Json(ApiResponse::success(session)))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tasks/{task_id}/sessions", get(list_sessions).post(create_session))
        .route("/tasks/{task_id}/sessions/{id}", get(get_session))
}
