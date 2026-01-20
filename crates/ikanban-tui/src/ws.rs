use anyhow::Result;
use futures_util::{stream::StreamExt, SinkExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::models::WsEvent;

/// Callback function type for handling WebSocket events
pub type EventCallback = Box<dyn Fn(WsEvent) + Send + 'static>;

/// WebSocket client for real-time updates
pub struct WebSocketClient {
    url: String,
    callback: Option<EventCallback>,
    /// Channel for receiving events to forward to the app
    event_tx: mpsc::UnboundedSender<WsEvent>,
    /// Handle to the WebSocket task
    _task_handle: tokio::task::JoinHandle<()>,
}

impl WebSocketClient {
    /// Create a new WebSocket client and connect to the given URL
    pub fn new(
        url: &str,
        event_tx: mpsc::UnboundedSender<WsEvent>,
    ) -> Self {
        let url = url.to_string();
        let event_tx_clone = event_tx.clone();
        let url_clone = url.clone();

        Self {
            url,
            callback: None,
            event_tx,
            _task_handle: tokio::spawn(async move {
                if let Err(e) = Self::connect_and_handle(&url_clone, event_tx_clone).await {
                    tracing::error!("WebSocket error: {}", e);
                }
            }),
        }
    }

    /// Set a callback function to be called when events are received
    pub fn set_callback(&mut self, callback: EventCallback) {
        self.callback = Some(callback);
    }

    /// Connect to WebSocket and handle incoming messages
    async fn connect_and_handle(
        url: &str,
        event_tx: mpsc::UnboundedSender<WsEvent>,
    ) -> Result<()> {
        let (ws_stream, _) = connect_async(url).await?;
        tracing::info!("WebSocket connected to {}", url);

        let (mut sender, mut read) = ws_stream.split();

        // Send initial connected message
        let connected_msg = serde_json::to_string(&WsEvent::Connected)?;
        sender
            .send(Message::Text(connected_msg.into()))
            .await?;

        // Forward events to the channel
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(event) = serde_json::from_str::<WsEvent>(&text) {
                        // Forward event to the app
                        let _ = event_tx.send(event.clone());
                        tracing::debug!("Received WebSocket event: {:?}", event);
                    } else {
                        tracing::warn!("Failed to parse WebSocket message: {}", text);
                    }
                }
                Ok(Message::Binary(data)) => {
                    tracing::debug!("Received binary WebSocket message: {} bytes", data.len());
                }
                Ok(Message::Ping(_)) => {
                    // Handle ping if needed
                }
                Ok(Message::Pong(_)) => {
                    // Handle pong if needed
                }
                Ok(Message::Close(_)) => {
                    tracing::info!("WebSocket connection closed");
                    break;
                }
                Ok(Message::Frame(_)) => {
                    // Ignore frame messages
                }
                Err(e) => {
                    tracing::error!("WebSocket error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Create a WebSocket client for projects stream
    pub fn projects(
        server_url: &str,
        event_tx: mpsc::UnboundedSender<WsEvent>,
    ) -> Self {
        let ws_base = server_url
            .replace("http://", "ws://")
            .replace("https://", "wss://");
        let url = format!("{}/api/projects/stream/ws", ws_base);
        Self::new(&url, event_tx)
    }

    /// Create a WebSocket client for tasks stream
    pub fn tasks(
        server_url: &str,
        project_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<WsEvent>,
    ) -> Self {
        let ws_base = server_url
            .replace("http://", "ws://")
            .replace("https://", "wss://");
        let url = format!(
            "{}/api/tasks/stream/ws?project_id={}",
            ws_base, project_id
        );
        Self::new(&url, event_tx)
    }

    /// Create a WebSocket client for execution logs stream
    pub fn execution_logs(
        server_url: &str,
        execution_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<WsEvent>,
    ) -> Self {
        let ws_base = server_url
            .replace("http://", "ws://")
            .replace("https://", "wss://");
        let url = format!(
            "{}/api/executions/{}/logs/stream",
            ws_base, execution_id
        );
        Self::new(&url, event_tx)
    }
}

/// Extension trait for displaying event types
trait EventTypeExt {
    fn event_type(&self) -> &'static str;
}

impl EventTypeExt for WsEvent {
    fn event_type(&self) -> &'static str {
        match self {
            WsEvent::ProjectCreated(_) => "ProjectCreated",
            WsEvent::ProjectUpdated(_) => "ProjectUpdated",
            WsEvent::ProjectDeleted { .. } => "ProjectDeleted",
            WsEvent::TaskCreated(_) => "TaskCreated",
            WsEvent::TaskUpdated(_) => "TaskUpdated",
            WsEvent::TaskDeleted { .. } => "TaskDeleted",
            WsEvent::SessionCreated(_) => "SessionCreated",
            WsEvent::SessionUpdated(_) => "SessionUpdated",
            WsEvent::ExecutionCreated(_) => "ExecutionCreated",
            WsEvent::ExecutionUpdated(_) => "ExecutionUpdated",
            WsEvent::DirectMergeCreated(_) => "DirectMergeCreated",
            WsEvent::PrMergeCreated(_) => "PrMergeCreated",
            WsEvent::PrMergeUpdated(_) => "PrMergeUpdated",
            WsEvent::Log { .. } => "Log",
            WsEvent::Connected => "Connected",
            WsEvent::Ping => "Ping",
            WsEvent::Pong => "Pong",
        }
    }
}
