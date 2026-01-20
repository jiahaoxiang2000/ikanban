use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub repo_path: Option<String>,
    pub archived: bool,
    pub pinned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProject {
    pub name: String,
    pub description: Option<String>,
    pub repo_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub description: Option<String>,
    pub repo_path: Option<String>,
    pub archived: Option<bool>,
    pub pinned: Option<bool>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ProjectWithStatus {
    #[sqlx(flatten)]
    pub project: Project,
    pub is_running: bool,
    pub is_errored: bool,
    pub task_count: i64,
    pub active_task_count: i64,
}

impl Project {
    pub async fn find_all(pool: &sqlx::SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM projects ORDER BY pinned DESC, updated_at DESC")
            .fetch_all(pool)
            .await
    }

    pub async fn find_all_with_status(pool: &sqlx::SqlitePool) -> Result<Vec<ProjectWithStatus>, sqlx::Error> {
        sqlx::query_as::<_, ProjectWithStatus>(
            r#"
            SELECT 
                p.*,
                (SELECT COUNT(*) FROM tasks t WHERE t.project_id = p.id) as task_count,
                (SELECT COUNT(*) FROM tasks t WHERE t.project_id = p.id AND t.status = 'in_progress') as active_task_count,
                0 as is_running,
                0 as is_errored
            FROM projects p
            ORDER BY p.pinned DESC, p.updated_at DESC
            "#
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_most_active(pool: &sqlx::SqlitePool) -> Result<Vec<ProjectWithStatus>, sqlx::Error> {
        sqlx::query_as::<_, ProjectWithStatus>(
            r#"
            SELECT 
                p.*,
                (SELECT COUNT(*) FROM tasks t WHERE t.project_id = p.id) as task_count,
                (SELECT COUNT(*) FROM tasks t WHERE t.project_id = p.id AND t.status = 'in_progress') as active_task_count,
                0 as is_running,
                0 as is_errored
            FROM projects p
            ORDER BY active_task_count DESC, p.updated_at DESC
            LIMIT 5
            "#
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_by_id(pool: &sqlx::SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn create(
        pool: &sqlx::SqlitePool,
        payload: &CreateProject,
    ) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO projects (id, name, description, repo_path, archived, pinned, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(&payload.name)
        .bind(&payload.description)
        .bind(&payload.repo_path)
        .bind(false) // archived
        .bind(false) // pinned
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        Ok(Self {
            id,
            name: payload.name.clone(),
            description: payload.description.clone(),
            repo_path: payload.repo_path.clone(),
            archived: false,
            pinned: false,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn update(
        pool: &sqlx::SqlitePool,
        id: Uuid,
        payload: &UpdateProject,
    ) -> Result<Option<Self>, sqlx::Error> {
        let existing = Self::find_by_id(pool, id).await?;
        let Some(existing) = existing else {
            return Ok(None);
        };

        let name = payload.name.as_ref().unwrap_or(&existing.name);
        let description = payload.description.as_ref().or(existing.description.as_ref());
        let repo_path = payload.repo_path.as_ref().or(existing.repo_path.as_ref());
        let archived = payload.archived.unwrap_or(existing.archived);
        let pinned = payload.pinned.unwrap_or(existing.pinned);
        let now = Utc::now();

        sqlx::query("UPDATE projects SET name = ?, description = ?, repo_path = ?, archived = ?, pinned = ?, updated_at = ? WHERE id = ?")
            .bind(name)
            .bind(description)
            .bind(repo_path)
            .bind(archived)
            .bind(pinned)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;

        Ok(Some(Self {
            id,
            name: name.clone(),
            description: description.cloned(),
            repo_path: repo_path.cloned(),
            archived,
            pinned,
            created_at: existing.created_at,
            updated_at: now,
        }))
    }

    pub async fn delete(pool: &sqlx::SqlitePool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
