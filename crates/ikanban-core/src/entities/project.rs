use chrono::Utc;
use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use serde::{Deserialize, Serialize};

use crate::entities::task;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "projects")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub repo_path: Option<String>,
    pub archived: bool,
    pub pinned: bool,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::task::Entity")]
    Tasks,
}

impl Related<super::task::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tasks.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// --- DTOs and Business Logic ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProject {
    pub name: String,
    pub description: Option<String>,
    pub repo_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub description: Option<String>,
    pub repo_path: Option<String>,
    pub archived: Option<bool>,
    pub pinned: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectWithStatus {
    #[serde(flatten)]
    pub project: Model,
    pub is_running: bool,
    pub is_errored: bool,
    pub task_count: i64,
    pub active_task_count: i64,
}

impl Model {
    pub async fn find_all(db: &DatabaseConnection) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .order_by_desc(Column::Pinned)
            .order_by_desc(Column::UpdatedAt)
            .all(db)
            .await
    }

    pub async fn find_all_with_status(
        db: &DatabaseConnection,
    ) -> Result<Vec<ProjectWithStatus>, DbErr> {
        let projects = Self::find_all(db).await?;
        let mut result = Vec::with_capacity(projects.len());

        for project in projects {
            let task_count = task::Entity::find()
                .filter(task::Column::ProjectId.eq(project.id))
                .count(db)
                .await? as i64;

            let active_task_count = task::Entity::find()
                .filter(task::Column::ProjectId.eq(project.id))
                .filter(task::Column::Status.eq(task::TaskStatus::InProgress))
                .count(db)
                .await? as i64;

            result.push(ProjectWithStatus {
                project,
                is_running: false,
                is_errored: false,
                task_count,
                active_task_count,
            });
        }

        Ok(result)
    }

    pub async fn find_most_active(
        db: &DatabaseConnection,
    ) -> Result<Vec<ProjectWithStatus>, DbErr> {
        let mut projects_with_status = Self::find_all_with_status(db).await?;

        // Sort by active_task_count desc, then updated_at desc
        projects_with_status.sort_by(|a, b| {
            b.active_task_count
                .cmp(&a.active_task_count)
                .then_with(|| b.project.updated_at.cmp(&a.project.updated_at))
        });

        projects_with_status.truncate(5);
        Ok(projects_with_status)
    }

    pub async fn find_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }

    pub async fn create(db: &DatabaseConnection, payload: &CreateProject) -> Result<Self, DbErr> {
        let now = Utc::now();
        let model = ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(payload.name.clone()),
            description: Set(payload.description.clone()),
            repo_path: Set(payload.repo_path.clone()),
            archived: Set(false),
            pinned: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        };

        model.insert(db).await
    }

    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        payload: &UpdateProject,
    ) -> Result<Option<Self>, DbErr> {
        let existing = Self::find_by_id(db, id).await?;
        let Some(existing) = existing else {
            return Ok(None);
        };

        let mut model: ActiveModel = existing.into();

        if let Some(name) = &payload.name {
            model.name = Set(name.clone());
        }
        if let Some(description) = &payload.description {
            model.description = Set(Some(description.clone()));
        }
        if let Some(repo_path) = &payload.repo_path {
            model.repo_path = Set(Some(repo_path.clone()));
        }
        if let Some(archived) = payload.archived {
            model.archived = Set(archived);
        }
        if let Some(pinned) = payload.pinned {
            model.pinned = Set(pinned);
        }
        model.updated_at = Set(Utc::now());

        let updated = model.update(db).await?;
        Ok(Some(updated))
    }

    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool, DbErr> {
        let result = Entity::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }
}
