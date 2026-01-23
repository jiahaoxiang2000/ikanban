use std::{collections::HashMap, path::Path, sync::Arc};

use serde_json::Value;

use super::types::{
    MessageRole, OpencodeExecutorEvent, Part, SdkEvent, SessionStatus, ToolPart, ToolStateUpdate,
};
use crate::executor::msg_store::{LogMsg, MsgStore};

/// Normalize OpenCode logs from MsgStore
///
/// This spawns a background task that processes stdout from the MsgStore,
/// parses OpenCode events, and converts them into structured log entries.
pub fn normalize_logs(msg_store: Arc<MsgStore>, _worktree_path: &Path) {
    tokio::spawn(async move {
        let mut state = LogState::new();

        // Get all existing messages
        for msg in msg_store.get_all() {
            if let LogMsg::Stdout(line) = msg {
                process_line(&mut state, &line);
            }
        }

        // Subscribe to new messages
        let mut rx = msg_store.subscribe();
        loop {
            match rx.recv().await {
                Ok(LogMsg::Stdout(line)) => {
                    process_line(&mut state, &line);
                }
                Ok(LogMsg::Finished) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // Re-process all messages if we lagged
                    state = LogState::new();
                    for msg in msg_store.get_all() {
                        if let LogMsg::Stdout(line) = msg {
                            process_line(&mut state, &line);
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                _ => {}
            }
        }
    });
}

fn process_line(state: &mut LogState, line: &str) {
    let Some(event) = parse_event(line) else {
        return;
    };

    match event {
        OpencodeExecutorEvent::SessionStart { session_id: _ } => {
            // Session ID already stored by SDK
        }
        OpencodeExecutorEvent::SdkEvent { event } => {
            state.handle_sdk_event(&event);
        }
        OpencodeExecutorEvent::Error { message: _ } => {
            // Error already logged
        }
        OpencodeExecutorEvent::Done => {
            // Done signal
        }
    }
}

fn parse_event(line: &str) -> Option<OpencodeExecutorEvent> {
    serde_json::from_str::<OpencodeExecutorEvent>(line.trim()).ok()
}

#[derive(Debug, Clone)]
struct StreamingText {
    content: String,
}

#[derive(Debug, Clone)]
enum UpdateMode {
    Append,
    Set,
}

#[derive(Default)]
struct LogState {
    message_roles: HashMap<String, MessageRole>,
    assistant_text: HashMap<String, StreamingText>,
    thinking_text: HashMap<String, StreamingText>,
    tool_states: HashMap<String, ToolCallState>,
}

impl LogState {
    fn new() -> Self {
        Self::default()
    }

    fn handle_sdk_event(&mut self, raw: &Value) {
        let Some(event) = SdkEvent::parse(raw) else {
            return;
        };

        match event {
            SdkEvent::MessageUpdated(event) => {
                let info = event.info;
                self.message_roles.insert(info.id, info.role);
            }
            SdkEvent::MessagePartUpdated(event) => {
                self.handle_part_update(event.part, event.delta.as_deref());
            }
            SdkEvent::SessionStatus(event) => {
                self.handle_session_status(event.status);
            }
            SdkEvent::SessionIdle => {}
            SdkEvent::SessionCompacted => {}
            SdkEvent::PermissionAsked(_) => {}
            SdkEvent::PermissionReplied
            | SdkEvent::MessageRemoved
            | SdkEvent::MessagePartRemoved
            | SdkEvent::CommandExecuted
            | SdkEvent::SessionDiff
            | SdkEvent::TuiSessionSelect
            | SdkEvent::TodoUpdated(_) => {}
            SdkEvent::SessionError(_) => {}
            SdkEvent::Unknown { .. } => {}
        }
    }

    fn handle_session_status(&mut self, status: SessionStatus) {
        match status {
            SessionStatus::Retry { .. } => {}
            SessionStatus::Idle | SessionStatus::Busy | SessionStatus::Other => {}
        }
    }

    fn handle_part_update(&mut self, part: Part, delta: Option<&str>) {
        match part {
            Part::Text(part) => {
                if self.message_roles.get(&part.message_id) != Some(&MessageRole::Assistant) {
                    return;
                }

                let (text, mode) = if let Some(delta) = delta {
                    (delta, UpdateMode::Append)
                } else {
                    (part.text.as_str(), UpdateMode::Set)
                };

                update_streaming_text(text, &part.message_id, &mut self.assistant_text, mode);
            }
            Part::Reasoning(part) => {
                let (text, mode) = if let Some(delta) = delta {
                    (delta, UpdateMode::Append)
                } else {
                    (part.text.as_str(), UpdateMode::Set)
                };

                update_streaming_text(text, &part.message_id, &mut self.thinking_text, mode);
            }
            Part::Tool(part) => {
                let part = *part;
                if part.call_id.trim().is_empty() {
                    return;
                }

                let tool_state = self
                    .tool_states
                    .entry(part.call_id.clone())
                    .or_insert_with(|| ToolCallState::new(part.call_id.clone()));

                tool_state.update_from_part(part);
            }
            Part::Other => {}
        }
    }
}

fn update_streaming_text(
    text: &str,
    message_id: &str,
    map: &mut HashMap<String, StreamingText>,
    mode: UpdateMode,
) {
    if text.is_empty() {
        return;
    }

    let is_new = !map.contains_key(message_id);

    if is_new && text == "\n" {
        return;
    }

    let state = map
        .entry(message_id.to_string())
        .or_insert_with(|| StreamingText {
            content: String::new(),
        });

    match mode {
        UpdateMode::Append => state.content.push_str(text),
        UpdateMode::Set => state.content = text.to_string(),
    }
}

#[derive(Debug, Clone)]
struct ToolCallState {
    #[allow(dead_code)]
    call_id: String,
    #[allow(dead_code)]
    tool_name: String,
    #[allow(dead_code)]
    state: ToolStateStatus,
    #[allow(dead_code)]
    title: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolStateStatus {
    Pending,
    Running,
    Completed,
    Error,
    Unknown,
}

impl ToolCallState {
    fn new(call_id: String) -> Self {
        Self {
            call_id,
            tool_name: "tool".to_string(),
            state: ToolStateStatus::Unknown,
            title: None,
        }
    }

    fn update_from_part(&mut self, part: ToolPart) {
        if !part.tool.is_empty() {
            self.tool_name = part.tool;
        }

        match &part.state {
            ToolStateUpdate::Pending { .. } => {
                self.state = ToolStateStatus::Pending;
            }
            ToolStateUpdate::Running { title, .. } => {
                self.state = ToolStateStatus::Running;
                if let Some(t) = title.as_ref().filter(|t| !t.trim().is_empty()) {
                    self.title = Some(t.clone());
                }
            }
            ToolStateUpdate::Completed { title, .. } => {
                self.state = ToolStateStatus::Completed;
                if let Some(t) = title.as_ref().filter(|t| !t.trim().is_empty()) {
                    self.title = Some(t.clone());
                }
            }
            ToolStateUpdate::Error { .. } => {
                self.state = ToolStateStatus::Error;
            }
            ToolStateUpdate::Unknown => {}
        }
    }
}
