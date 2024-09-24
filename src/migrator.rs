use cardamon::migrations;
use sea_orm_migration::prelude::*;

#[tokio::main]
async fn main() {
    cli::run_cli(migrations::Migrator).await;
}
