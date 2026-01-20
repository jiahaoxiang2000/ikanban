use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
    QueryOrder, Set,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::entities::{task, Task as TaskEntity};

// Re-export from entity
pub use task::TaskStatus;
pub type Task = task::Model;

#[derive(Debug, Deserialize)]
pub struct CreateTask {
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
}

#[derive(Debug, Deserialize)]
pub struct TaskQuery {
    pub project_id: Uuid,
}

impl Task {
    pub async fn find_by_project_id(
        db: &DatabaseConnection,
        project_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        TaskEntity::find()
            .filter(task::Column::ProjectId.eq(project_id))
            .order_by_desc(task::Column::CreatedAt)
            .all(db)
            .await
    }

    pub async fn find_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<Self>, DbErr> {
        TaskEntity::find_by_id(id).one(db).await
    }

    pub async fn create(db: &DatabaseConnection, payload: &CreateTask) -> Result<Self, DbErr> {
        let now = Utc::now();
        let model = task::ActiveModel {
            id: Set(Uuid::new_v4()),
            project_id: Set(payload.project_id),
            title: Set(payload.title.clone()),
            description: Set(payload.description.clone()),
            status: Set(payload.status.unwrap_or_default()),
            created_at: Set(now),
            updated_at: Set(now),
        };

        model.insert(db).await
    }

    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        payload: &UpdateTask,
    ) -> Result<Option<Self>, DbErr> {
        let existing = Self::find_by_id(db, id).await?;
        let Some(existing) = existing else {
            return Ok(None);
        };

        let mut model: task::ActiveModel = existing.into();

        if let Some(title) = &payload.title {
            model.title = Set(title.clone());
        }
        if let Some(description) = &payload.description {
            model.description = Set(Some(description.clone()));
        }
        if let Some(status) = payload.status {
            model.status = Set(status);
        }
        model.updated_at = Set(Utc::now());

        let updated = model.update(db).await?;
        Ok(Some(updated))
    }

    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool, DbErr> {
        let result = TaskEntity::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    pub async fn delete_by_project_id(
        db: &DatabaseConnection,
        project_id: Uuid,
    ) -> Result<u64, DbErr> {
        let result = TaskEntity::delete_many()
            .filter(task::Column::ProjectId.eq(project_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }
}
