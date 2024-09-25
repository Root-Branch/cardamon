use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PowerCurve::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PowerCurve::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PowerCurve::A).double().not_null())
                    .col(ColumnDef::new(PowerCurve::B).double().not_null())
                    .col(ColumnDef::new(PowerCurve::C).double().not_null())
                    .col(ColumnDef::new(PowerCurve::D).double().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Cpu::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Cpu::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Cpu::Name).string().not_null())
                    .col(ColumnDef::new(Cpu::Tdp).double())
                    .col(ColumnDef::new(Cpu::PowerCurveId).integer())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Cpu::Table, Cpu::PowerCurveId)
                            .to(PowerCurve::Table, PowerCurve::Id),
                    )
                    .to_owned(),
            )
            .await?;

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
                    .col(
                        ColumnDef::new(Run::IsLive)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(Run::CpuId).integer().not_null())
                    .col(ColumnDef::new(Run::StartTime).big_integer().not_null())
                    .col(ColumnDef::new(Run::StopTime).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Run::Table, Run::CpuId)
                            .to(Cpu::Table, Cpu::Id),
                    )
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
pub enum PowerCurve {
    Table,
    Id,
    A,
    B,
    C,
    D,
}

#[derive(DeriveIden)]
pub enum Cpu {
    Table,
    Id,
    Name,
    Tdp,
    PowerCurveId,
}

#[derive(DeriveIden)]
pub enum Run {
    Table,
    Id,
    IsLive,
    CpuId,
    StartTime,
    StopTime,
}
