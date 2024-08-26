pub mod m20240822_095823_create_run_table;
pub mod m20240822_095830_create_metrics_table;
pub mod m20240822_095838_create_iteration_table;

pub use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240822_095823_create_run_table::Migration),
            Box::new(m20240822_095830_create_metrics_table::Migration),
            Box::new(m20240822_095838_create_iteration_table::Migration),
        ]
    }
}
