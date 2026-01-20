use axum::{
    extract::{Path, State, WebSocketUpgrade},
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{ApiResponse, CreateProject, Project, ProjectWithStatus, UpdateProject, WsEvent},
    AppState,
};

/// GET /api/projects - List all projects
pub async fn list_projects(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ProjectWithStatus>>>, AppError> {
    let projects = Project::find_all_with_status(&state.db).await?;
    Ok(Json(ApiResponse::success(projects)))
}

/// GET /api/projects/active - List most active projects
pub async fn list_active_projects(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ProjectWithStatus>>>, AppError> {
    let projects = Project::find_most_active(&state.db).await?;
    Ok(Json(ApiResponse::success(projects)))
}

/// POST /api/projects - Create a new project
pub async fn create_project(
    State(state): State<AppState>,
    Json(payload): Json<CreateProject>,
) -> Result<Json<ApiResponse<Project>>, AppError> {
    if payload.name.trim().is_empty() {
        return Err(AppError::BadRequest("Project name cannot be empty".to_string()));
    }

    let project = Project::create(&state.db, &payload).await?;

    // Broadcast event
    state.broadcast(WsEvent::ProjectCreated(project.clone()));

    tracing::info!("Created project: {} ({})", project.name, project.id);
    Ok(Json(ApiResponse::success(project)))
}

/// GET /api/projects/{id} - Get project by ID
pub async fn get_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Project>>, AppError> {
    let project = Project::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project {} not found", id)))?;

    Ok(Json(ApiResponse::success(project)))
}

/// PUT /api/projects/{id} - Update project
pub async fn update_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateProject>,
) -> Result<Json<ApiResponse<Project>>, AppError> {
    let project = Project::update(&state.db, id, &payload)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project {} not found", id)))?;

    // Broadcast event
    state.broadcast(WsEvent::ProjectUpdated(project.clone()));

    tracing::info!("Updated project: {} ({})", project.name, project.id);
    Ok(Json(ApiResponse::success(project)))
}

/// DELETE /api/projects/{id} - Delete project
pub async fn delete_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    // First delete all tasks associated with this project
    let tasks_deleted = crate::models::Task::delete_by_project_id(&state.db, id).await?;
    tracing::debug!("Deleted {} tasks for project {}", tasks_deleted, id);

    let deleted = Project::delete(&state.db, id).await?;

    if !deleted {
        return Err(AppError::NotFound(format!("Project {} not found", id)));
    }

    // Broadcast event
    state.broadcast(WsEvent::ProjectDeleted { id });

    tracing::info!("Deleted project: {}", id);
    Ok(Json(ApiResponse::success(())))
}

/// GET /api/projects/stream/ws - WebSocket stream for project updates
pub async fn stream_projects_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_projects_ws(socket, state).await {
            tracing::warn!("Projects WebSocket closed: {}", e);
        }
    })
}

async fn handle_projects_ws(
    socket: axum::extract::ws::WebSocket,
    state: AppState,
) -> anyhow::Result<()> {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = state.subscribe();

    // Send initial connected message
    let connected_msg = serde_json::to_string(&WsEvent::Connected)?;
    sender
        .send(axum::extract::ws::Message::Text(connected_msg.into()))
        .await?;

    // Spawn task to handle incoming messages (for ping/pong)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(_msg)) = receiver.next().await {
            // Ping handling is automatic in axum
        }
    });

    // Forward project events to client
    loop {
        match event_rx.recv().await {
            Ok(event) => {
                // Only forward project-related events
                let should_send = matches!(
                    event,
                    WsEvent::ProjectCreated(_)
                        | WsEvent::ProjectUpdated(_)
                        | WsEvent::ProjectDeleted { .. }
                );

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

    recv_task.abort();
    Ok(())
}

use tokio::sync::broadcast;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects", get(list_projects).post(create_project))
        .route("/projects/active", get(list_active_projects))
        .route("/projects/stream/ws", get(stream_projects_ws))
        .route(
            "/projects/{id}",
            get(get_project).put(update_project).delete(delete_project),
        )
}
