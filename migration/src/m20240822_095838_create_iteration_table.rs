use crate::m20240822_095823_create_run_table::Run;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Iteration::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Iteration::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Iteration::RunId).integer().not_null())
                    .col(ColumnDef::new(Iteration::ScenarioName).string().not_null())
                    .col(ColumnDef::new(Iteration::Count).integer().not_null())
                    .col(
                        ColumnDef::new(Iteration::StartTime)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Iteration::StopTime).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Iteration::Table, Iteration::RunId)
                            .to(Run::Table, Run::Id),
                    )
                    // unique constraint
                    .index(
                        Index::create()
                            .name("unique trio")
                            .col(Iteration::RunId)
                            .col(Iteration::ScenarioName)
                            .col(Iteration::Count)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Iteration::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Iteration {
    Table,
    Id,
    RunId,
    ScenarioName,
    Count,
    StartTime,
    StopTime,
}
