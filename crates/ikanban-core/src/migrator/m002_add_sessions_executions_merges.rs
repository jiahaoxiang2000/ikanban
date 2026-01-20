use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create sessions table
        manager
            .create_table(
                Table::create()
                    .table(Sessions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Sessions::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Sessions::TaskId).uuid().not_null())
                    .col(ColumnDef::new(Sessions::Executor).string().not_null()) // e.g., "agent", "user"
                    .col(ColumnDef::new(Sessions::Status).string().not_null())
                    .col(ColumnDef::new(Sessions::StartedAt).timestamp())
                    .col(ColumnDef::new(Sessions::CompletedAt).timestamp())
                    .col(ColumnDef::new(Sessions::CreatedAt).timestamp().not_null())
                    .col(ColumnDef::new(Sessions::UpdatedAt).timestamp().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sessions_task_id")
                            .from(Sessions::Table, Sessions::TaskId)
                            .to(Tasks::Table, Tasks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create execution_processes table
        manager
            .create_table(
                Table::create()
                    .table(ExecutionProcesses::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ExecutionProcesses::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(ExecutionProcesses::SessionId).uuid().not_null())
                    .col(ColumnDef::new(ExecutionProcesses::RunReason).string().not_null())
                    .col(ColumnDef::new(ExecutionProcesses::ExecutorAction).string())
                    .col(ColumnDef::new(ExecutionProcesses::Status).string().not_null())
                    .col(ColumnDef::new(ExecutionProcesses::ExitCode).integer())
                    .col(ColumnDef::new(ExecutionProcesses::Dropped).boolean().default(false))
                    .col(ColumnDef::new(ExecutionProcesses::StartedAt).timestamp())
                    .col(ColumnDef::new(ExecutionProcesses::CompletedAt).timestamp())
                    .col(ColumnDef::new(ExecutionProcesses::CreatedAt).timestamp().not_null())
                    .col(ColumnDef::new(ExecutionProcesses::UpdatedAt).timestamp().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_execution_processes_session_id")
                            .from(ExecutionProcesses::Table, ExecutionProcesses::SessionId)
                            .to(Sessions::Table, Sessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create direct_merges table
        manager
            .create_table(
                Table::create()
                    .table(DirectMerges::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(DirectMerges::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(DirectMerges::ProjectId).uuid().not_null())
                    .col(ColumnDef::new(DirectMerges::MergeCommit).string().not_null())
                    .col(ColumnDef::new(DirectMerges::TargetBranch).string().not_null())
                    .col(ColumnDef::new(DirectMerges::CreatedAt).timestamp().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_direct_merges_project_id")
                            .from(DirectMerges::Table, DirectMerges::ProjectId)
                            .to(Projects::Table, Projects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create pr_merges table
        manager
            .create_table(
                Table::create()
                    .table(PrMerges::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(PrMerges::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(PrMerges::ProjectId).uuid().not_null())
                    .col(ColumnDef::new(PrMerges::TargetBranch).string().not_null())
                    .col(ColumnDef::new(PrMerges::PrNumber).integer().not_null())
                    .col(ColumnDef::new(PrMerges::PrUrl).string().not_null())
                    .col(ColumnDef::new(PrMerges::Status).string().not_null())
                    .col(ColumnDef::new(PrMerges::MergedAt).timestamp())
                    .col(ColumnDef::new(PrMerges::CreatedAt).timestamp().not_null())
                    .col(ColumnDef::new(PrMerges::UpdatedAt).timestamp().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_pr_merges_project_id")
                            .from(PrMerges::Table, PrMerges::ProjectId)
                            .to(Projects::Table, Projects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PrMerges::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(DirectMerges::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ExecutionProcesses::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Sessions::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Projects {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Tasks {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Sessions {
    Table,
    Id,
    TaskId,
    Executor,
    Status,
    StartedAt,
    CompletedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ExecutionProcesses {
    Table,
    Id,
    SessionId,
    RunReason,
    ExecutorAction,
    Status,
    ExitCode,
    Dropped,
    StartedAt,
    CompletedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum DirectMerges {
    Table,
    Id,
    ProjectId,
    MergeCommit,
    TargetBranch,
    CreatedAt,
}

#[derive(DeriveIden)]
enum PrMerges {
    Table,
    Id,
    ProjectId,
    TargetBranch,
    PrNumber,
    PrUrl,
    Status,
    MergedAt,
    CreatedAt,
    UpdatedAt,
}
