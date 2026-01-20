use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "execution_processes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub session_id: Uuid,
    pub run_reason: String,
    pub executor_action: Option<String>,
    pub status: String,
    pub exit_code: Option<i32>,
    pub dropped: bool,
    pub started_at: Option<DateTime>,
    pub completed_at: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::session::Entity",
        from = "Column::SessionId",
        to = "super::session::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Session,
}

impl Related<super::session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Session.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// --- DTOs and Business Logic ---

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateExecutionProcess {
    pub session_id: Uuid,
    pub run_reason: String,
    pub executor_action: Option<String>,
}

impl Model {
    pub async fn find_by_session_id(
        db: &DatabaseConnection,
        session_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::SessionId.eq(session_id))
            .order_by_desc(Column::CreatedAt)
            .all(db)
            .await
    }

    pub async fn find_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }

    pub async fn create(
        db: &DatabaseConnection,
        payload: &CreateExecutionProcess,
    ) -> Result<Self, DbErr> {
        let now = chrono::Utc::now().naive_utc();
        let model = ActiveModel {
            id: Set(Uuid::new_v4()),
            session_id: Set(payload.session_id),
            run_reason: Set(payload.run_reason.clone()),
            executor_action: Set(payload.executor_action.clone()),
            status: Set("running".to_string()),
            dropped: Set(false),
            started_at: Set(Some(now)),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        model.insert(db).await
    }

    pub async fn stop(db: &DatabaseConnection, id: Uuid) -> Result<Option<Self>, DbErr> {
        let existing = Self::find_by_id(db, id).await?;
        let Some(existing) = existing else {
            return Ok(None);
        };

        let mut model: ActiveModel = existing.into();
        let now = chrono::Utc::now().naive_utc();
        
        model.status = Set("killed".to_string());
        model.completed_at = Set(Some(now));
        model.updated_at = Set(now);

        let updated = model.update(db).await?;
        Ok(Some(updated))
    }
}
