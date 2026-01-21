use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub prompt: String,
    pub model: Option<String>,
    pub max_turns: Option<u32>,
}
