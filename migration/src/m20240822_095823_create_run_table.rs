use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Run::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Run::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Run::StartTime).big_integer().not_null())
                    .col(ColumnDef::new(Run::StopTime).big_integer())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Run::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Run {
    Table,
    Id,
    StartTime,
    StopTime,
}
