use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub task_id: Uuid,
    pub executor: String,
    pub status: String,
    pub started_at: Option<DateTime>,
    pub completed_at: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::task::Entity",
        from = "Column::TaskId",
        to = "super::task::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Task,
    #[sea_orm(has_many = "super::execution_process::Entity")]
    ExecutionProcess,
}

impl Related<super::task::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Task.def()
    }
}

impl Related<super::execution_process::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ExecutionProcess.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// --- DTOs and Business Logic ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSession {
    pub task_id: Uuid,
    pub executor: String,
}

#[derive(Debug, Deserialize)]
pub struct SessionQuery {
    pub task_id: Uuid,
}

impl Model {
    pub async fn find_by_task_id(
        db: &DatabaseConnection,
        task_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::TaskId.eq(task_id))
            .order_by_desc(Column::CreatedAt)
            .all(db)
            .await
    }

    pub async fn find_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }

    pub async fn create(db: &DatabaseConnection, payload: &CreateSession) -> Result<Self, DbErr> {
        let now = chrono::Utc::now().naive_utc();
        let model = ActiveModel {
            id: Set(Uuid::new_v4()),
            task_id: Set(payload.task_id),
            executor: Set(payload.executor.clone()),
            status: Set("running".to_string()), // Default status
            started_at: Set(Some(now)),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        model.insert(db).await
    }
}
