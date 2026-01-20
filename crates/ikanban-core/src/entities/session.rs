use sea_orm::entity::prelude::*;
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
