use crate::server::db::DbPool;
use crate::server::models::{AgentConfig, ExecutionProcess, ExecutionStatus, Session, SessionStatus, Task};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Task not found: {0}")]
    TaskNotFound(String),
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    #[error("Project not found: {0}")]
    ProjectNotFound(String),
    #[error("Invalid state transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },
    #[error("Worktree error: {0}")]
    Worktree(String),
    #[error("Agent error: {0}")]
    Agent(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type SessionResult<T> = Result<T, SessionError>;

struct RunningProcess {
    child: Child,
    execution_id: String,
}

pub struct SessionManager {
    pool: DbPool,
    running_processes: Arc<RwLock<HashMap<String, RunningProcess>>>,
    worktree_base: PathBuf,
}

impl SessionManager {
    pub fn new(pool: DbPool) -> Self {
        let worktree_base = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ikanban")
            .join("worktrees");

        Self {
            pool,
            running_processes: Arc::new(RwLock::new(HashMap::new())),
            worktree_base,
        }
    }

    pub async fn create_worktree_session(
        &self,
        task_id: &str,
        _project_path: &str,
        branch_name: Option<&str>,
    ) -> SessionResult<Session> {
        let task = self.get_task(task_id).await?;
        let project = self.get_project(&task.project_id).await?;

        let branch = branch_name
            .map(String::from)
            .unwrap_or_else(|| self.generate_branch_name(&task));

        let worktree_path = self.worktree_base.join(format!("task-{}", task_id));
        let worktree_path_str = worktree_path.to_string_lossy().to_string();

        self.create_git_worktree(&project.repo_path, &worktree_path_str, &branch)
            .await?;

        let session_id = self.generate_id();
        let now = chrono_now();

        sqlx::query(
            r#"
            INSERT INTO sessions (id, task_id, worktree_path, branch_name, status, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&session_id)
        .bind(task_id)
        .bind(&worktree_path_str)
        .bind(&branch)
        .bind(SessionStatus::Pending.as_str())
        .bind(&now)
        .execute(&self.pool)
        .await?;

        self.get_session(&session_id).await
    }

    pub async fn start_agent(
        &self,
        session_id: &str,
        config: AgentConfig,
    ) -> SessionResult<ExecutionProcess> {
        let session = self.get_session(session_id).await?;

        self.transition_session_status(session_id, SessionStatus::Running)
            .await?;

        let execution_id = self.generate_id();
        let now = chrono_now();

        sqlx::query(
            r#"
            INSERT INTO execution_processes (id, session_id, run_reason, status, started_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&execution_id)
        .bind(session_id)
        .bind("coding_agent")
        .bind(ExecutionStatus::Running.as_str())
        .bind(&now)
        .execute(&self.pool)
        .await?;

        let worktree_path = session
            .worktree_path
            .ok_or_else(|| SessionError::Worktree("Session has no worktree path".into()))?;

        let child = self
            .spawn_agent_process(&worktree_path, &config, &execution_id)
            .await?;

        let pid = child.id().map(|p| p as i64);

        if let Some(pid) = pid {
            sqlx::query("UPDATE execution_processes SET pid = ? WHERE id = ?")
                .bind(pid)
                .bind(&execution_id)
                .execute(&self.pool)
                .await?;
        }

        {
            let mut processes = self.running_processes.write().await;
            processes.insert(
                session_id.to_string(),
                RunningProcess {
                    child,
                    execution_id: execution_id.clone(),
                },
            );
        }

        self.get_execution(&execution_id).await
    }

    pub async fn stop_session(&self, session_id: &str) -> SessionResult<()> {
        let session = self.get_session(session_id).await?;
        let current_status = session.status_enum();

        if current_status == SessionStatus::Running {
            if let Some(mut process) = self.running_processes.write().await.remove(session_id) {
                let _ = process.child.kill().await;

                let now = chrono_now();
                sqlx::query(
                    "UPDATE execution_processes SET status = ?, ended_at = ? WHERE id = ?",
                )
                .bind(ExecutionStatus::Cancelled.as_str())
                .bind(&now)
                .bind(&process.execution_id)
                .execute(&self.pool)
                .await?;
            }

            self.transition_session_status(session_id, SessionStatus::Cancelled)
                .await?;
        }

        Ok(())
    }

    pub async fn transition_session_status(
        &self,
        session_id: &str,
        new_status: SessionStatus,
    ) -> SessionResult<Session> {
        let session = self.get_session(session_id).await?;
        let current_status = session.status_enum();

        if !current_status.can_transition_to(new_status) {
            return Err(SessionError::InvalidTransition {
                from: current_status.as_str().to_string(),
                to: new_status.as_str().to_string(),
            });
        }

        let now = chrono_now();
        let ended_at = match new_status {
            SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Cancelled => {
                Some(now.clone())
            }
            _ => None,
        };

        if let Some(ref ended) = ended_at {
            sqlx::query("UPDATE sessions SET status = ?, ended_at = ? WHERE id = ?")
                .bind(new_status.as_str())
                .bind(ended)
                .bind(session_id)
                .execute(&self.pool)
                .await?;
        } else {
            sqlx::query("UPDATE sessions SET status = ? WHERE id = ?")
                .bind(new_status.as_str())
                .bind(session_id)
                .execute(&self.pool)
                .await?;
        }

        self.get_session(session_id).await
    }

    pub async fn get_session(&self, session_id: &str) -> SessionResult<Session> {
        sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = ?")
            .bind(session_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| SessionError::SessionNotFound(session_id.to_string()))
    }

    pub async fn get_sessions_for_task(&self, task_id: &str) -> SessionResult<Vec<Session>> {
        let sessions =
            sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE task_id = ? ORDER BY created_at DESC")
                .bind(task_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(sessions)
    }

    pub async fn mark_execution_completed(&self, execution_id: &str) -> SessionResult<()> {
        let now = chrono_now();
        sqlx::query("UPDATE execution_processes SET status = ?, ended_at = ? WHERE id = ?")
            .bind(ExecutionStatus::Completed.as_str())
            .bind(&now)
            .bind(execution_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark_execution_failed(&self, execution_id: &str) -> SessionResult<()> {
        let now = chrono_now();
        sqlx::query("UPDATE execution_processes SET status = ?, ended_at = ? WHERE id = ?")
            .bind(ExecutionStatus::Failed.as_str())
            .bind(&now)
            .bind(execution_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_task(&self, task_id: &str) -> SessionResult<Task> {
        sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = ?")
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| SessionError::TaskNotFound(task_id.to_string()))
    }

    async fn get_project(&self, project_id: &str) -> SessionResult<crate::server::models::Project> {
        sqlx::query_as::<_, crate::server::models::Project>("SELECT * FROM projects WHERE id = ?")
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| SessionError::ProjectNotFound(project_id.to_string()))
    }

    async fn get_execution(&self, execution_id: &str) -> SessionResult<ExecutionProcess> {
        sqlx::query_as::<_, ExecutionProcess>("SELECT * FROM execution_processes WHERE id = ?")
            .bind(execution_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| SessionError::SessionNotFound(execution_id.to_string()))
    }

    async fn create_git_worktree(
        &self,
        repo_path: &str,
        worktree_path: &str,
        branch_name: &str,
    ) -> SessionResult<()> {
        if let Some(parent) = PathBuf::from(worktree_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let output = Command::new("git")
            .args(["worktree", "add", "-b", branch_name, worktree_path])
            .current_dir(repo_path)
            .output()
            .await?;

        if !output.status.success() {
            let output_retry = Command::new("git")
                .args(["worktree", "add", worktree_path, branch_name])
                .current_dir(repo_path)
                .output()
                .await?;

            if !output_retry.status.success() {
                let stderr = String::from_utf8_lossy(&output_retry.stderr);
                return Err(SessionError::Worktree(format!(
                    "Failed to create worktree: {}",
                    stderr
                )));
            }
        }

        Ok(())
    }

    async fn spawn_agent_process(
        &self,
        worktree_path: &str,
        config: &AgentConfig,
        execution_id: &str,
    ) -> SessionResult<Child> {
        let mut cmd = Command::new("claude");
        cmd.arg("--print")
            .arg("--dangerously-skip-permissions")
            .arg(&config.prompt)
            .current_dir(worktree_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(ref model) = config.model {
            cmd.arg("--model").arg(model);
        }

        let mut child = cmd.spawn().map_err(|e| {
            SessionError::Agent(format!("Failed to spawn agent process: {}", e))
        })?;

        let pool = self.pool.clone();
        let exec_id = execution_id.to_string();

        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                let mut turn_number = 0;

                while let Ok(Some(line)) = lines.next_line().await {
                    turn_number += 1;
                    let turn_id = format!("{}-turn-{}", exec_id, turn_number);
                    let now = chrono_now();

                    let _ = sqlx::query(
                        r#"
                        INSERT INTO coding_agent_turns (id, execution_id, turn_number, output, created_at)
                        VALUES (?, ?, ?, ?, ?)
                        "#,
                    )
                    .bind(&turn_id)
                    .bind(&exec_id)
                    .bind(turn_number)
                    .bind(&line)
                    .bind(&now)
                    .execute(&pool)
                    .await;
                }
            });
        }

        Ok(child)
    }

    fn generate_branch_name(&self, task: &Task) -> String {
        let slug: String = task
            .title
            .chars()
            .filter_map(|c| {
                if c.is_alphanumeric() {
                    Some(c.to_ascii_lowercase())
                } else if c.is_whitespace() || c == '-' {
                    Some('-')
                } else {
                    None
                }
            })
            .take(30)
            .collect();

        format!("task/{}-{}", task.id, slug)
    }

    fn generate_id(&self) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let random: u32 = rand_u32();
        format!("{:x}{:08x}", timestamp, random)
    }
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    let secs = duration.as_secs();
    let datetime = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        1970 + secs / 31536000,
        (secs % 31536000) / 2592000 + 1,
        (secs % 2592000) / 86400 + 1,
        (secs % 86400) / 3600,
        (secs % 3600) / 60,
        secs % 60
    );
    datetime
}

fn rand_u32() -> u32 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    RandomState::new().build_hasher().finish() as u32
}
