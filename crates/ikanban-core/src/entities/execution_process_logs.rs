use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryOrder, QuerySelect, QueryFilter, Set};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "execution_process_logs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub execution_process_id: Uuid,
    pub level: String, // "info", "warn", "error", "debug", "trace"
    pub message: String,
    pub timestamp: DateTime,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::execution_process::Entity",
        from = "Column::ExecutionProcessId",
        to = "super::execution_process::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    ExecutionProcess,
}

impl Related<super::execution_process::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ExecutionProcess.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Log levels supported
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }

    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "warn" => LogLevel::Warn,
            "error" => LogLevel::Error,
            "debug" => LogLevel::Debug,
            "trace" => LogLevel::Trace,
            _ => LogLevel::Info,
        }
    }
}

// --- DTOs and Business Logic ---

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateExecutionProcessLog {
    pub execution_process_id: Uuid,
    pub level: String,
    pub message: String,
}

impl Model {
    pub async fn find_by_execution_process_id(
        db: &DatabaseConnection,
        execution_process_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::ExecutionProcessId.eq(execution_process_id))
            .order_by_asc(Column::Timestamp)
            .all(db)
            .await
    }

    pub async fn find_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }

    pub async fn create(
        db: &DatabaseConnection,
        payload: &CreateExecutionProcessLog,
    ) -> Result<Self, DbErr> {
        let now = chrono::Utc::now().naive_utc();
        let model = ActiveModel {
            id: Set(Uuid::new_v4()),
            execution_process_id: Set(payload.execution_process_id),
            level: Set(payload.level.clone()),
            message: Set(payload.message.clone()),
            timestamp: Set(now),
            created_at: Set(now),
        };

        model.insert(db).await
    }

    /// Create a batch of log entries efficiently
    pub async fn create_batch(
        db: &DatabaseConnection,
        logs: &[CreateExecutionProcessLog],
    ) -> Result<(), DbErr> {
        let now = chrono::Utc::now().naive_utc();
        let mut active_models = Vec::new();

        for log in logs {
            let model = ActiveModel {
                id: Set(Uuid::new_v4()),
                execution_process_id: Set(log.execution_process_id),
                level: Set(log.level.clone()),
                message: Set(log.message.clone()),
                timestamp: Set(now),
                created_at: Set(now),
            };
            active_models.push(model);
        }

        Entity::insert_many(active_models).exec(db).await?;
        Ok(())
    }

    /// Get recent logs for an execution process with limit
    pub async fn find_recent_by_execution_process_id(
        db: &DatabaseConnection,
        execution_process_id: Uuid,
        limit: u64,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::ExecutionProcessId.eq(execution_process_id))
            .order_by_desc(Column::Timestamp)
            .limit(limit)
            .all(db)
            .await
    }
}
