use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Run::Table)
                    .add_column_if_not_exists(ColumnDef::new(Run::Region).string())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Run::Table)
                    .drop_column(Alias::new("region"))
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
pub enum Run {
    Table,
    Region,
    CarbonIntensity,
}
