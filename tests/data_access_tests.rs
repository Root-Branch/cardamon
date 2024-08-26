#![allow(unused_imports, dead_code)]

use anyhow::{self, Context};
use cardamon::{
    dao,
    migrations::{Migrator, MigratorTrait},
};
use itertools::*;
use sea_orm::{
    entity::*, error::*, query::*, sea_query, tests_cfg::*, Database, DatabaseBackend,
    DatabaseConnection, DbConn,
};
use std::path::Path;

async fn setup_db() -> anyhow::Result<DatabaseConnection> {
    // Connecting SQLite
    let db = Database::connect("sqlite::memory:").await?;

    // Migrate db
    Migrator::up(&db, None)
        .await
        .context("Error migrating database.")
        .ok();

    Ok(db)
}

async fn setup_fixtures(fixtures: &[&str], db: &DatabaseConnection) -> anyhow::Result<()> {
    for path in fixtures {
        let path = Path::new(path);
        let stmt = std::fs::read_to_string(path)?;
        db.query_one(Statement::from_string(DatabaseBackend::Sqlite, stmt))
            .await
            .context(format!("Error applying fixture {:?}", path))?;
    }

    Ok(())
}

#[tokio::test]
async fn fetch_iterations_of_last_n_runs_for_schema() -> anyhow::Result<()> {
    let db = setup_db().await?;
    setup_fixtures(&["./fixtures/runs.sql", "./fixtures/iterations.sql"], &db).await?;

    // fetch the latest scenario_1 run
    let scenario_iterations = dao::iteration::fetch_runs_last_n("scenario_1", 1, &db).await?;

    let run_ids = scenario_iterations
        .iter()
        .map(|run| run.run_id)
        .collect::<Vec<_>>();
    assert_eq!(run_ids, vec![1]);

    let iterations = scenario_iterations
        .iter()
        .map(|run| run.count)
        .collect::<Vec<_>>();
    assert_eq!(iterations, vec![1]);

    // fetch the last 2 scenario_3 runs
    let scenario_iterations = dao::iteration::fetch_runs_last_n("scenario_3", 2, &db).await?;

    let run_ids = scenario_iterations
        .iter()
        .map(|run| run.run_id)
        .collect::<Vec<_>>();
    assert_eq!(run_ids, vec![2, 2, 2, 3, 3, 3]);

    let iterations = scenario_iterations
        .iter()
        .map(|run| run.count)
        .collect::<Vec<_>>();
    assert_eq!(iterations, vec![1, 2, 3, 1, 2, 3]);

    Ok(())
}

#[tokio::test]
async fn fetch_metrics_within() -> anyhow::Result<()> {
    let db = setup_db().await?;
    setup_fixtures(&["./fixtures/runs.sql", "./fixtures/metrics.sql"], &db).await?;

    let metrics = dao::metrics::fetch_within(1, 1717507600000, 1717507600200, &db).await?;

    assert_eq!(metrics.len(), 4);

    let process_names: Vec<&str> = metrics
        .iter()
        .map(|metric| metric.process_name.as_str())
        .unique()
        .collect();

    assert_eq!(process_names, vec!["yarn", "docker"]);

    Ok(())
}
