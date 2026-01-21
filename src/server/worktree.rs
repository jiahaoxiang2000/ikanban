use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorktreeError {
    #[error("Git command failed: {0}")]
    GitError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Worktree not found: {0}")]
    NotFound(String),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

#[derive(Debug, Clone)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    pub commit: String,
}

fn worktrees_base_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ikanban")
        .join("worktrees")
}

fn worktree_path_for_task(task_id: i64) -> PathBuf {
    worktrees_base_dir().join(format!("task-{}", task_id))
}

pub fn generate_branch_name(task_id: i64, slug: &str) -> String {
    let sanitized_slug: String = slug
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .to_lowercase();

    let trimmed = sanitized_slug.trim_matches('-');
    let truncated = if trimmed.len() > 50 {
        &trimmed[..50]
    } else {
        trimmed
    };

    format!("task/{}-{}", task_id, truncated)
}

pub fn create_worktree(
    project_path: &Path,
    task_id: i64,
    branch_name: &str,
) -> Result<PathBuf, WorktreeError> {
    let worktree_path = worktree_path_for_task(task_id);

    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let output = Command::new("git")
        .args(["worktree", "add", "-b", branch_name])
        .arg(&worktree_path)
        .current_dir(project_path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("already exists") {
            let output = Command::new("git")
                .args(["worktree", "add"])
                .arg(&worktree_path)
                .arg(branch_name)
                .current_dir(project_path)
                .output()?;

            if !output.status.success() {
                return Err(WorktreeError::GitError(
                    String::from_utf8_lossy(&output.stderr).to_string(),
                ));
            }
        } else {
            return Err(WorktreeError::GitError(stderr.to_string()));
        }
    }

    Ok(worktree_path)
}

pub fn remove_worktree(worktree_path: &Path) -> Result<(), WorktreeError> {
    if !worktree_path.exists() {
        return Err(WorktreeError::NotFound(worktree_path.display().to_string()));
    }

    let output = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(worktree_path)
        .output()?;

    if !output.status.success() {
        return Err(WorktreeError::GitError(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(())
}

pub fn list_worktrees(project_path: &Path) -> Result<Vec<Worktree>, WorktreeError> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(project_path)
        .output()?;

    if !output.status.success() {
        return Err(WorktreeError::GitError(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_commit: Option<String> = None;
    let mut current_branch: Option<String> = None;

    for line in stdout.lines() {
        if let Some(path_str) = line.strip_prefix("worktree ") {
            if let (Some(path), Some(commit), Some(branch)) = (
                current_path.take(),
                current_commit.take(),
                current_branch.take(),
            ) {
                worktrees.push(Worktree {
                    path,
                    commit,
                    branch,
                });
            }
            current_path = Some(PathBuf::from(path_str));
        } else if let Some(commit) = line.strip_prefix("HEAD ") {
            current_commit = Some(commit.to_string());
        } else if let Some(branch) = line.strip_prefix("branch ") {
            current_branch = Some(branch.replace("refs/heads/", ""));
        } else if line == "detached" {
            current_branch = Some("(detached)".to_string());
        }
    }

    if let (Some(path), Some(commit), Some(branch)) = (current_path, current_commit, current_branch)
    {
        worktrees.push(Worktree {
            path,
            commit,
            branch,
        });
    }

    Ok(worktrees)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_branch_name() {
        assert_eq!(
            generate_branch_name(123, "Fix login bug"),
            "task/123-fix-login-bug"
        );
        assert_eq!(
            generate_branch_name(1, "Hello World!"),
            "task/1-hello-world"
        );
        assert_eq!(generate_branch_name(42, "simple"), "task/42-simple");
    }

    #[test]
    fn test_worktree_path_for_task() {
        let path = worktree_path_for_task(123);
        assert!(path.ends_with("task-123"));
    }
}
