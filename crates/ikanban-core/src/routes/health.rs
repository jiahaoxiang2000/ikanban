use axum::{Json, Router, routing::get};

use crate::{AppState, entities::response::ApiResponse};

pub async fn health_check() -> Json<ApiResponse<String>> {
    Json(ApiResponse::success("OK".to_string()))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health_check))
}
