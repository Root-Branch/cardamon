//! `SeaORM` Entity, @generated by sea-orm-codegen 1.0.0

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "metrics")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub run_id: i32,
    pub process_id: String,
    pub process_name: String,
    #[sea_orm(column_type = "Double")]
    pub cpu_usage: f64,
    #[sea_orm(column_type = "Double")]
    pub cpu_total_usage: f64,
    pub cpu_core_count: i32,
    pub time_stamp: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::run::Entity",
        from = "Column::RunId",
        to = "super::run::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Run,
}

impl Related<super::run::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Run.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
