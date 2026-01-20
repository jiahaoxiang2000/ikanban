mod health;
mod projects;
mod tasks;
mod events;
mod executions;

use axum::Router;

use crate::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(health::router())
        .nest("/api", api_router())
        .with_state(state)
}

fn api_router() -> Router<AppState> {
    Router::new()
        .merge(projects::router())
        .merge(tasks::router())
        .merge(events::router())
        .merge(executions::router())
}
