pub mod carbon_intensity;
pub mod config;
pub mod dao;
pub mod data;
pub mod entities;
pub mod execution_modes;
pub mod metrics;
pub mod metrics_logger;
pub mod migrations;
pub mod models;
pub mod server;

use crate::{
    execution_modes::{execution_plan::ExecutionPlan, ExecutionMode},
    migrations::{Migrator, MigratorTrait},
};
use anyhow::Context;
use colored::Colorize;
use config::Power;
use entities::cpu;
use execution_modes::{
    live_monitor::run_live,
    process_control::{run_process, shutdown_processes},
    scenario_runner::run_scenarios,
};
use sea_orm::*;
use std::{
    fs::{self},
    io::Write,
    path::Path,
    process::exit,
};
use tracing::debug;

pub async fn db_connect(
    database_url: &str,
    database_name: Option<&str>,
) -> anyhow::Result<DatabaseConnection> {
    let db = Database::connect(database_url).await?;
    match db.get_database_backend() {
        DbBackend::Sqlite => Ok(db),

        DbBackend::Postgres => {
            let database_name =
                database_name.context("Database name is required for postgres connections")?;
            db.execute(Statement::from_string(
                db.get_database_backend(),
                format!("CREATE DATABASE \"{}\";", database_name),
            ))
            .await
            .ok();

            let url = format!("{}/{}", database_url, database_name);
            Database::connect(&url)
                .await
                .context("Error creating postgresql database.")
        }

        DbBackend::MySql => {
            let database_name =
                database_name.context("Database name is required for mysql connections")?;
            db.execute(Statement::from_string(
                db.get_database_backend(),
                format!("CREATE DATABASE IF NOT EXISTS `{}`;", database_name),
            ))
            .await?;

            let url = format!("{}/{}", database_url, database_name);
            Database::connect(&url)
                .await
                .context("Error creating mysql database.")
        }
    }
}

pub async fn db_migrate(db_conn: &DatabaseConnection) -> anyhow::Result<()> {
    Migrator::up(db_conn, None)
        .await
        .context("Error migrating database.")
}

/// Deletes previous runs .stdout and .stderr
/// Stdout and stderr capturing are append due to a scenario / observeration removing previous ones
/// stdout and err
pub fn cleanup_stdout_stderr() -> anyhow::Result<()> {
    debug!("Cleaning up stdout and stderr");
    let stdout = Path::new("./.stdout");
    let stderr = Path::new("./.stderr");
    if stdout.exists() {
        fs::remove_file(stdout)?;
    }
    if stderr.exists() {
        fs::remove_file(stderr)?;
    }
    Ok(())
}

pub async fn run(
    exec_plan: ExecutionPlan<'_>,
    region: &Option<String>,
    ci: f64,
    db: &DatabaseConnection,
) -> anyhow::Result<()> {
    let mut processes_to_observe = exec_plan.external_processes_to_observe.unwrap_or(vec![]); // external procs to observe are cloned here.

    // run the application if there is anything to run
    if !exec_plan.processes_to_execute.is_empty() {
        for proc in exec_plan.processes_to_execute {
            print!("> starting process {}", proc.name.green());

            let process_to_observe = run_process(proc)?;

            // add process_to_observe to the observation list
            processes_to_observe.push(process_to_observe);
            println!("{}", "\t✓".green());
            println!("\t{}", format!("- {}", proc.up).bright_black());
        }
    }

    print!("> waiting for application to settle");
    std::io::stdout().flush()?;
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    println!(" {}", "\t✓".green());

    // check if the processor already exists in the db.
    // If it does then reuse it for this run else save
    // a new one
    let cpu = cpu::Entity::find()
        .filter(cpu::Column::Name.eq(&exec_plan.cpu.name))
        .one(db)
        .await?;

    let cpu_id = match cpu {
        Some(cpu) => cpu.id,
        None => {
            let cpu = match exec_plan.cpu.power {
                Power::Tdp(tdp) => {
                    cpu::ActiveModel {
                        id: ActiveValue::NotSet,
                        name: ActiveValue::Set(exec_plan.cpu.name),
                        tdp: ActiveValue::Set(Some(tdp)),
                        power_curve_id: ActiveValue::NotSet,
                    }
                    .save(db)
                    .await
                }

                Power::Curve(a, b, c, d) => {
                    let power_curve = entities::power_curve::ActiveModel {
                        id: ActiveValue::NotSet,
                        a: ActiveValue::Set(a),
                        b: ActiveValue::Set(b),
                        c: ActiveValue::Set(c),
                        d: ActiveValue::Set(d),
                    }
                    .save(db)
                    .await?
                    .try_into_model()?;

                    cpu::ActiveModel {
                        id: ActiveValue::NotSet,
                        name: ActiveValue::Set(exec_plan.cpu.name),
                        tdp: ActiveValue::NotSet,
                        power_curve_id: ActiveValue::Set(Some(power_curve.id)),
                    }
                    .save(db)
                    .await
                }
            }?;

            cpu.try_into_model()?.id
        }
    };

    // gracefully shutdown upon ctrl-c
    let processes_to_shutdown = processes_to_observe.clone();
    ctrlc::set_handler(move || {
        println!();
        shutdown_processes(&processes_to_shutdown).expect("Error shutting down managed processes");
        exit(0)
    })?;

    match exec_plan.execution_mode {
        ExecutionMode::Observation(scenarios) => {
            run_scenarios(
                cpu_id,
                region,
                ci,
                scenarios,
                processes_to_observe.clone(),
                db,
            )
            .await?;
        }

        ExecutionMode::Live => {
            run_live(cpu_id, region, ci, processes_to_observe.clone(), db).await?;
        }

        ExecutionMode::Daemon => {
            todo!()
        }
    };

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub async fn setup_fixtures(fixtures: &[&str], db: &DatabaseConnection) -> anyhow::Result<()> {
        for path in fixtures {
            let path = Path::new(path);
            let stmt = std::fs::read_to_string(path)?;
            db.query_one(Statement::from_string(DatabaseBackend::Sqlite, stmt))
                .await
                .context(format!("Error applying fixture {:?}", path))?;
        }

        Ok(())
    }
}
