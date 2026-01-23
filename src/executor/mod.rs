use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::{mpsc, oneshot};

/// Environment configuration for executor
#[derive(Debug, Clone)]
pub struct ExecutionEnv {
    pub repo_paths: Vec<PathBuf>,
    pub env_vars: HashMap<String, String>,
}

impl ExecutionEnv {
    pub fn new() -> Self {
        Self {
            repo_paths: Vec::new(),
            env_vars: HashMap::new(),
        }
    }

    pub fn with_repo_path(mut self, path: PathBuf) -> Self {
        self.repo_paths.push(path);
        self
    }

    pub fn with_env_var(mut self, key: String, value: String) -> Self {
        self.env_vars.insert(key, value);
        self
    }
}

impl Default for ExecutionEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle to a spawned child process
pub struct SpawnedChild {
    /// The actual child process
    pub child: tokio::process::Child,
    /// Executor → Container: signals when executor wants to exit
    pub exit_signal: Option<oneshot::Receiver<()>>,
    /// Container → Executor: signals when container wants to interrupt
    pub interrupt_sender: Option<mpsc::Sender<()>>,
}

/// Trait for executor implementations
#[async_trait]
pub trait Executor: Send + Sync {
    /// Spawn agent server and return child process handle
    async fn spawn(
        &self,
        working_dir: &Path,
        prompt: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild>;

    /// Spawn follow-up in existing session
    async fn spawn_follow_up(
        &self,
        working_dir: &Path,
        prompt: &str,
        session_id: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild>;

    /// Get the executor type identifier
    fn executor_type(&self) -> &str;
}

// Module exports
pub mod msg_store;
pub mod opencode;

pub use msg_store::{LogMsg, MsgStore};
