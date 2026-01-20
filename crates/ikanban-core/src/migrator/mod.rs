use sea_orm_migration::prelude::*;

mod m001_create_tables;
mod m002_add_sessions_executions_merges;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m001_create_tables::Migration),
            Box::new(m002_add_sessions_executions_merges::Migration),
        ]
    }
}
