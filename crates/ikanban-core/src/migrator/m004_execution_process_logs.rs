use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create execution_process_logs table
        manager
            .create_table(
                Table::create()
                    .table(ExecutionProcessLogs::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ExecutionProcessLogs::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(ExecutionProcessLogs::ExecutionProcessId).uuid().not_null())
                    .col(ColumnDef::new(ExecutionProcessLogs::Level).string().not_null())
                    .col(ColumnDef::new(ExecutionProcessLogs::Message).string().not_null())
                    .col(ColumnDef::new(ExecutionProcessLogs::Timestamp).timestamp().not_null())
                    .col(ColumnDef::new(ExecutionProcessLogs::CreatedAt).timestamp().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_execution_process_logs_execution_process_id")
                            .from(ExecutionProcessLogs::Table, ExecutionProcessLogs::ExecutionProcessId)
                            .to(ExecutionProcesses::Table, ExecutionProcesses::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index for faster queries by execution_process_id and timestamp
        manager
            .create_index(
                Index::create()
                    .name("idx_execution_process_logs_execution_process_id")
                    .table(ExecutionProcessLogs::Table)
                    .col(ExecutionProcessLogs::ExecutionProcessId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_execution_process_logs_timestamp")
                    .table(ExecutionProcessLogs::Table)
                    .col(ExecutionProcessLogs::Timestamp)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("idx_execution_process_logs_timestamp").to_owned())
            .await?;

        manager
            .drop_index(Index::drop().name("idx_execution_process_logs_execution_process_id").to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(ExecutionProcessLogs::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum ExecutionProcesses {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ExecutionProcessLogs {
    Table,
    Id,
    ExecutionProcessId,
    Level,
    Message,
    Timestamp,
    CreatedAt,
}
