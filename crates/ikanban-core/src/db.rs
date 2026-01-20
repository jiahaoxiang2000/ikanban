use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;
use std::time::Duration;

use crate::migrator::Migrator;

pub async fn create_connection(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut opt = ConnectOptions::new(database_url);
    opt.max_connections(5)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(10))
        .acquire_timeout(Duration::from_secs(10))
        .sqlx_logging(false);

    let db = Database::connect(opt).await?;

    // Run migrations automatically
    Migrator::up(&db, None).await?;

    tracing::info!("Database migrations completed");
    Ok(db)
}
