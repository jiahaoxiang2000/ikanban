use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, QueryOrder, Set,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    #[sea_orm(string_value = "todo")]
    Todo,
    #[sea_orm(string_value = "inprogress")]
    InProgress,
    #[sea_orm(string_value = "done")]
    Done,
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Todo
    }
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::project::Entity",
        from = "Column::ProjectId",
        to = "super::project::Column::Id",
        on_delete = "Cascade"
    )]
    Project,
}

impl Related<super::project::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Project.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// --- DTOs and Business Logic ---

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTask {
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
}

#[derive(Debug, Deserialize)]
pub struct TaskQuery {
    pub project_id: Uuid,
}

impl Model {
    pub async fn find_by_project_id(
        db: &DatabaseConnection,
        project_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::ProjectId.eq(project_id))
            .order_by_desc(Column::CreatedAt)
            .all(db)
            .await
    }

    pub async fn find_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }

    pub async fn create(db: &DatabaseConnection, payload: &CreateTask) -> Result<Self, DbErr> {
        let now = Utc::now();
        let model = ActiveModel {
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

        let mut model: ActiveModel = existing.into();

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
        let result = Entity::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    pub async fn delete_by_project_id(
        db: &DatabaseConnection,
        project_id: Uuid,
    ) -> Result<u64, DbErr> {
        let result = Entity::delete_many()
            .filter(Column::ProjectId.eq(project_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }
}
