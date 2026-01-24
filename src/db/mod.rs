pub mod models;

pub mod connection {
    use anyhow::Result;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
    use std::path::Path;
    use std::str::FromStr;

    pub async fn create_pool(db_path: &Path) -> Result<SqlitePool> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let db_url = format!("sqlite:{}", db_path.display());
        let options = SqliteConnectOptions::from_str(&db_url)?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        // Run migrations
        run_migrations(&pool).await?;

        Ok(pool)
    }

    async fn run_migrations(pool: &SqlitePool) -> Result<()> {
        // Enable foreign keys
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(pool)
            .await?;

        // Create projects table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create tasks table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create sessions table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                worktree_path TEXT,
                branch_name TEXT,
                executor_type TEXT NOT NULL,
                status TEXT NOT NULL,
                exit_code INTEGER,
                created_at TEXT NOT NULL,
                started_at TEXT,
                finished_at TEXT,
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create log_entries table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS log_entries (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                log_type TEXT NOT NULL,
                content TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create indices for better query performance
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_tasks_project_id
            ON tasks(project_id)
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_sessions_task_id
            ON sessions(task_id)
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_log_entries_session_id
            ON log_entries(session_id)
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_log_entries_timestamp
            ON log_entries(timestamp)
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use tempfile::tempdir;

        #[tokio::test]
        async fn test_create_pool() {
            let dir = tempdir().unwrap();
            let db_path = dir.path().join("test.db");

            let pool = create_pool(&db_path).await.unwrap();

            // Verify tables exist
            let result: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('projects', 'tasks', 'sessions', 'log_entries')"
            )
            .fetch_one(&pool)
            .await
            .unwrap();

            assert_eq!(result.0, 4);
        }

        #[tokio::test]
        async fn test_foreign_keys_enabled() {
            let dir = tempdir().unwrap();
            let db_path = dir.path().join("test.db");

            let pool = create_pool(&db_path).await.unwrap();

            let result: (i64,) = sqlx::query_as("PRAGMA foreign_keys")
                .fetch_one(&pool)
                .await
                .unwrap();

            assert_eq!(result.0, 1);
        }
    }
}
