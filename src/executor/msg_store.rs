use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

/// Log message types that can be collected during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogMsg {
    /// Standard output from the process
    Stdout(String),
    /// Standard error from the process
    Stderr(String),
    /// SDK event (JSON serialized)
    Event(String),
    /// Session ID announcement
    SessionId(String),
    /// Execution finished signal
    Finished,
}

/// Message store for collecting and broadcasting executor logs
///
/// This provides:
/// - In-memory storage of all log messages
/// - Broadcasting mechanism for real-time log streaming
/// - Thread-safe access via RwLock
pub struct MsgStore {
    messages: RwLock<Vec<LogMsg>>,
    notify: broadcast::Sender<LogMsg>,
}

impl MsgStore {
    /// Create a new MsgStore wrapped in an Arc for sharing
    pub fn new() -> Arc<Self> {
        let (notify, _) = broadcast::channel(1024);
        Arc::new(Self {
            messages: RwLock::new(Vec::new()),
            notify,
        })
    }

    /// Push a new log message to the store and broadcast to subscribers
    pub fn push(&self, msg: LogMsg) {
        if let Ok(mut messages) = self.messages.write() {
            messages.push(msg.clone());
        }
        let _ = self.notify.send(msg);
    }

    /// Get a snapshot of all messages currently in the store
    pub fn get_all(&self) -> Vec<LogMsg> {
        self.messages.read().unwrap().clone()
    }

    /// Subscribe to receive new messages as they arrive
    pub fn subscribe(&self) -> broadcast::Receiver<LogMsg> {
        self.notify.subscribe()
    }

    /// Clear all stored messages
    pub fn clear(&self) {
        if let Ok(mut messages) = self.messages.write() {
            messages.clear();
        }
    }
}

impl Default for MsgStore {
    fn default() -> Self {
        let (notify, _) = broadcast::channel(1024);
        Self {
            messages: RwLock::new(Vec::new()),
            notify,
        }
    }
}
