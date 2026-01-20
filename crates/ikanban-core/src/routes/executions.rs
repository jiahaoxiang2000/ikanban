use axum::{
    extract::{Path, State, ws::{Message, WebSocket, WebSocketUpgrade}},
    response::Response,
    routing::get,
    Router,
};
use uuid::Uuid;

use crate::AppState;
use crate::entities::response::WsEvent;

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

pub fn router() -> Router<AppState> {
    Router::new().route("/executions/:id/logs/stream", get(logs_stream))
}
