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
                    .add_column(
                        ColumnDef::new(Run::CarbonIntensity)
                            .double()
                            .not_null()
                            .default(0.494),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Run::Table)
                    .drop_column(Alias::new("carbon_intensity"))
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Run {
    Table,
    CarbonIntensity,
}
