use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Event envelope structure from OpenCode SDK
#[derive(Debug, Deserialize)]
pub(crate) struct SdkEventEnvelope {
    #[serde(rename = "type")]
    pub(crate) type_: String,
    #[serde(default)]
    pub(crate) properties: Value,
}

/// Parsed SDK events from OpenCode
#[derive(Debug, Clone)]
pub enum SdkEvent {
    /// Message content updated
    MessageUpdated(MessageUpdatedEvent),
    /// Message part updated (streaming)
    MessagePartUpdated(MessagePartUpdatedEvent),
    /// Message removed
    MessageRemoved,
    /// Message part removed
    MessagePartRemoved,
    /// Permission requested by agent
    PermissionAsked(PermissionAskedEvent),
    /// Permission replied
    PermissionReplied,
    /// Session became idle
    SessionIdle,
    /// Session status changed
    SessionStatus(SessionStatusEvent),
    /// Session diff event
    SessionDiff,
    /// Session compacted
    SessionCompacted,
    /// Session error occurred
    SessionError(SessionErrorEvent),
    /// Todo list updated
    TodoUpdated(TodoUpdatedEvent),
    /// Command executed
    CommandExecuted,
    /// TUI session select event
    TuiSessionSelect,
    /// Unknown event type
    Unknown { type_: String, properties: Value },
}

impl SdkEvent {
    /// Parse a JSON value into an SdkEvent
    pub(crate) fn parse(value: &Value) -> Option<Self> {
        let envelope = serde_json::from_value::<SdkEventEnvelope>(value.clone()).ok()?;

        let event = match envelope.type_.as_str() {
            "message.updated" => {
                SdkEvent::MessageUpdated(serde_json::from_value(envelope.properties).ok()?)
            }
            "message.part.updated" => {
                SdkEvent::MessagePartUpdated(serde_json::from_value(envelope.properties).ok()?)
            }
            "message.removed" => SdkEvent::MessageRemoved,
            "message.part.removed" => SdkEvent::MessagePartRemoved,
            "permission.asked" => {
                SdkEvent::PermissionAsked(serde_json::from_value(envelope.properties).ok()?)
            }
            "permission.replied" => SdkEvent::PermissionReplied,
            "session.idle" => SdkEvent::SessionIdle,
            "session.status" => {
                SdkEvent::SessionStatus(serde_json::from_value(envelope.properties).ok()?)
            }
            "session.diff" => SdkEvent::SessionDiff,
            "session.compacted" => SdkEvent::SessionCompacted,
            "session.error" => {
                SdkEvent::SessionError(serde_json::from_value(envelope.properties).ok()?)
            }
            "todo.updated" => {
                SdkEvent::TodoUpdated(serde_json::from_value(envelope.properties).ok()?)
            }
            "command.executed" => SdkEvent::CommandExecuted,
            "tui.session.select" => SdkEvent::TuiSessionSelect,
            _ => SdkEvent::Unknown {
                type_: envelope.type_,
                properties: envelope.properties,
            },
        };

        Some(event)
    }
}

/// Message role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}

/// Message updated event
#[derive(Debug, Clone, Deserialize)]
pub struct MessageUpdatedEvent {
    pub info: MessageInfo,
}

/// Message information
#[derive(Debug, Clone, Deserialize)]
pub struct MessageInfo {
    pub id: String,
    pub role: MessageRole,
    #[serde(default)]
    pub model: Option<MessageModelInfo>,
    #[serde(rename = "providerID", default)]
    pub provider_id: Option<String>,
    #[serde(rename = "modelID", default)]
    pub model_id: Option<String>,
}

impl MessageInfo {
    pub fn provider_id(&self) -> Option<&str> {
        self.model
            .as_ref()
            .map(|m| m.provider_id.as_str())
            .or(self.provider_id.as_deref())
    }

    pub fn model_id(&self) -> Option<&str> {
        self.model
            .as_ref()
            .map(|m| m.model_id.as_str())
            .or(self.model_id.as_deref())
    }
}

/// Model information
#[derive(Debug, Clone, Deserialize)]
pub struct MessageModelInfo {
    #[serde(rename = "providerID", alias = "providerId")]
    pub provider_id: String,
    #[serde(rename = "modelID", alias = "modelId")]
    pub model_id: String,
}

/// Message part updated event (streaming)
#[derive(Debug, Clone, Deserialize)]
pub struct MessagePartUpdatedEvent {
    pub part: Part,
    #[serde(default)]
    pub delta: Option<String>,
}

/// Permission asked event
#[derive(Debug, Clone, Deserialize)]
pub struct PermissionAskedEvent {
    pub id: String,
    pub permission: String,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub tool: Option<PermissionToolInfo>,
}

/// Tool information in permission request
#[derive(Debug, Clone, Deserialize)]
pub struct PermissionToolInfo {
    #[serde(rename = "callID")]
    pub call_id: String,
}

/// Session status event
#[derive(Debug, Clone, Deserialize)]
pub struct SessionStatusEvent {
    pub status: SessionStatus,
}

/// Session status variants
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SessionStatus {
    Idle,
    Busy,
    Retry {
        attempt: u64,
        message: String,
        next: u64,
    },
    #[serde(other)]
    Other,
}

/// Todo updated event
#[derive(Debug, Clone, Deserialize)]
pub struct TodoUpdatedEvent {
    pub todos: Vec<SdkTodo>,
}

/// Todo item from SDK
#[derive(Debug, Clone, Deserialize)]
pub struct SdkTodo {
    pub id: String,
    pub content: String,
    pub status: String,
    pub priority: String,
}

/// Message part types
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum Part {
    #[serde(rename = "text")]
    Text(TextPart),
    #[serde(rename = "reasoning")]
    Reasoning(ReasoningPart),
    #[serde(rename = "tool")]
    Tool(Box<ToolPart>),
    #[serde(other)]
    Other,
}

/// Text part
#[derive(Debug, Clone, Deserialize)]
pub struct TextPart {
    #[serde(rename = "messageID")]
    pub message_id: String,
    pub text: String,
}

/// Reasoning part (same structure as TextPart)
pub type ReasoningPart = TextPart;

/// Tool part
#[derive(Debug, Clone, Deserialize)]
pub struct ToolPart {
    #[serde(rename = "messageID")]
    pub message_id: String,
    #[serde(rename = "callID")]
    pub call_id: String,
    #[serde(default)]
    pub tool: String,
    pub state: ToolStateUpdate,
}

/// Tool state update
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum ToolStateUpdate {
    Pending {
        #[serde(default)]
        input: Option<Value>,
    },
    Running {
        #[serde(default)]
        input: Option<Value>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        metadata: Option<Value>,
    },
    Completed {
        #[serde(default)]
        input: Option<Value>,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        metadata: Option<Value>,
    },
    Error {
        #[serde(default)]
        input: Option<Value>,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        metadata: Option<Value>,
    },
    #[serde(other)]
    Unknown,
}

/// Session error event
#[derive(Debug, Clone, Deserialize)]
pub struct SessionErrorEvent {
    #[serde(default)]
    pub error: Option<SdkError>,
}

/// SDK error information
#[derive(Debug, Clone)]
pub struct SdkError {
    pub raw: Value,
}

impl SdkError {
    pub fn kind(&self) -> &str {
        self.raw
            .get("name")
            .or_else(|| self.raw.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
    }

    pub fn message(&self) -> Option<String> {
        self.raw
            .pointer("/data/message")
            .or_else(|| self.raw.get("message"))
            .and_then(Value::as_str)
            .map(|s| s.to_string())
    }
}

impl<'de> Deserialize<'de> for SdkError {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = Value::deserialize(deserializer)?;
        Ok(Self { raw })
    }
}

/// High-level executor event types for external consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpencodeExecutorEvent {
    SessionStart {
        session_id: String,
    },
    SdkEvent {
        event: serde_json::Value,
    },
    Error {
        message: String,
    },
    Done,
}
