use super::m20240822_095823_create_run_table::Run;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Metrics::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Metrics::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Metrics::RunId).integer().not_null())
                    .col(ColumnDef::new(Metrics::ProcessId).string().not_null())
                    .col(ColumnDef::new(Metrics::ProcessName).string().not_null())
                    .col(ColumnDef::new(Metrics::CpuUsage).double().not_null())
                    .col(ColumnDef::new(Metrics::CpuTotalUsage).double().not_null())
                    .col(ColumnDef::new(Metrics::CpuCoreCount).integer().not_null())
                    .col(ColumnDef::new(Metrics::TimeStamp).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Metrics::Table, Metrics::RunId)
                            .to(Run::Table, Run::Id),
                    )
                    // unique constraint
                    .index(
                        Index::create()
                            .col(Metrics::RunId)
                            .col(Metrics::ProcessId)
                            .col(Metrics::TimeStamp)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Metrics::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Metrics {
    Table,
    Id,
    RunId,
    ProcessId,
    ProcessName,
    CpuUsage,
    CpuTotalUsage,
    CpuCoreCount,
    TimeStamp,
}
