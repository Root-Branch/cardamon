/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

extern crate dotenv;

pub mod metrics_server;
pub mod scenario_runner;
pub mod telegraf;

use crate::metrics_server::dto::{ScenarioRunStats, ScenarioSummaryOpts};
use anyhow::Result;
use clap::{Parser, Subcommand};
use diesel::{prelude::*, SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenv::dotenv;
use nanoid::nanoid;
use std::collections::HashMap;
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;

type DB = diesel::sqlite::Sqlite;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

#[derive(Parser)]
#[command(author = "Oliver Winks (@ohuu)", version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to telegraf conf
    #[arg(short, long)]
    conf: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run scenarios
    Run {
        /// Path to scenario scripts
        path: Option<PathBuf>,

        #[arg(long, default_value_t = 12)]
        cpu_tdp: u16,

        #[arg(long, default_value_t = 160)]
        carbon_intensity: u16,
    },
}
fn run_migrations(db_conn: &mut impl MigrationHarness<DB>) {
    db_conn
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to migrate the cardamon database!");
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    env_logger::init();

    let database_url = dotenv::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // check if cardamon database has been built
    if !Path::new(&database_url).exists() {
        if File::create(&database_url).is_err() {
            panic!("Error creating database file")
        }
    }

    // start sqlite connection and migrate the db
    let mut db_conn = SqliteConnection::establish(&database_url)?;
    run_migrations(&mut db_conn);

    let cli = Cli::parse();

    // run the scenarios
    match &cli.command {
        Commands::Run {
            path,
            cpu_tdp: _,
            carbon_intensity: _,
        } => {
            // create a unique cardamon label for this scenario
            let cardamon_run_id = nanoid!();
            let cardamon_run_type = "SCENARIO";

            // create state
            let state = Arc::new(Mutex::new(db_conn));

            // start the metric server
            metrics_server::start(state.clone());

            // start telegraf
            let conf_path = cli.conf.unwrap_or(PathBuf::from("telegraf.conf"));
            telegraf::start(
                conf_path,
                String::from("SCENARIO"),
                cardamon_run_id.clone(),
                String::from("http://localhost:8000"),
            );

            // wait a second for telegraf to start
            sleep(Duration::from_millis(1000)).await;

            // run scenarios
            let default_path = PathBuf::from("scenarios");
            let scenarios_path = path.as_ref().unwrap_or(&default_path);
            let scenarios =
                scenario_runner::run(scenarios_path, &cardamon_run_type, &cardamon_run_id).await?;

            // generate carbon summary
            let summary_opts = ScenarioSummaryOpts {
                scenarios,
                last_n: 3,
                carbon_intensity: HashMap::from([(String::from("accuheat_client_db"), 180.0)]),
                cpu_tdp: HashMap::from([(String::from("accuheat_client_db"), 23.0)]),
            };

            let res = ureq::get("http://localhost:8000/scenario_summary")
                .send_json(ureq::json!(summary_opts))?;

            let stats = res.into_json::<Vec<ScenarioRunStats>>();
            if let Ok(stats) = stats {
                for stats in stats {
                    println!("\n+ {}", stats.scenario_name);
                    for stats in stats.run_stats {
                        println!("\t + [{}]", stats.start_time.format("%Y-%m-%d @ %H:%M"));
                        for stats in stats.process_stats {
                            println!(
                                "\t\t {:}: \t{:.2} W \t{:.5} mgCO2eq",
                                stats.process_name,
                                stats.energy_consumption_w,
                                stats.carbon_emissions_g * 1000.0
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
