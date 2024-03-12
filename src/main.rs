/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

extern crate dotenv;

pub mod metrics_server;
pub mod scenario_runner;
pub mod telegraf;

use clap::{command, Args, Parser, Subcommand};
use diesel::{prelude::*, SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenv::dotenv;
use log::info;
use nanoid::nanoid;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;

use crate::metrics_server::dto;

type DB = diesel::sqlite::Sqlite;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

#[derive(Parser)]
#[command(author = "Oliver Winks (@ohuu)", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scenario commands
    Scenario(ScenarioArgs),

    /// Telemetry server commands
    #[command(visible_alias = "server")]
    TelemetryServer(TelemetryServerArgs),
}

#[derive(Args)]
struct ScenarioArgs {
    #[command(subcommand)]
    command: ScenarioCommands,

    /// Path to scenario scripts
    #[arg(long, short, required = true)]
    path: Option<PathBuf>,

    /// Path to telegraf conf
    #[arg(long, short)]
    telegraf_conf: Option<PathBuf>,
}

#[derive(Subcommand)]
enum ScenarioCommands {
    Run {
        #[arg(long, short)]
        scenario: Option<String>,
    },
}

#[derive(Args)]
struct TelemetryServerArgs {
    #[command(subcommand)]
    command: TelemetryServerCommands,
}

#[derive(Subcommand)]
enum TelemetryServerCommands {
    Run {
        /// Port to start server on
        #[arg(long, short, default_value_t = 2050)]
        port: i32,
    },
}

fn run_migrations(db_conn: &mut impl MigrationHarness<DB>) {
    db_conn
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to migrate the cardamon database!");
}

async fn init_scenario_run(
    state: Arc<Mutex<SqliteConnection>>,
    telegraf_conf_path: PathBuf,
) -> anyhow::Result<String> {
    // create a unique cardamon label for this scenario
    let cardamon_run_id = nanoid!();
    let cardamon_run_type = String::from("SCENARIO");

    // start the metric server
    metrics_server::start(state);

    // start telegraf
    telegraf::start(
        telegraf_conf_path,
        cardamon_run_type,
        cardamon_run_id.clone(),
        String::from("http://localhost:2050"),
    );

    // wait a second for telegraf to start
    sleep(Duration::from_millis(1000)).await;

    Ok(cardamon_run_id)
}

fn generate_scenario_summary(scenarios: Vec<String>) -> anyhow::Result<String> {
    // generate carbon summary
    let summary_opts = dto::ScenarioSummaryOpts {
        scenarios,
        last_n: 3,
        cpu_tdp: 23.0,
    };

    // request summary of scenario run compared to previous runs
    let stats = ureq::get("http://localhost:2050/scenario_summary")
        .send_json(ureq::json!(summary_opts))
        .map(|res| res.into_json::<Vec<dto::ScenarioRunStats>>())?;

    stats
        .map(|stats| {
            let mut summary = String::new();
            for stats in stats {
                summary.push_str(&format!("\n{}", stats.scenario_name));
                for stats in stats.run_stats {
                    summary.push_str(&format!(
                        "\n\t + [{}]",
                        stats.start_time.format("%Y-%m-%d @ %H:%M")
                    ));
                    for stats in stats.process_stats {
                        summary.push_str(&format!(
                            "\n\t\t {:}: \t{:.2}",
                            stats.process_name, stats.energy_consumption_w,
                        ));
                    }
                }
            }

            summary
        })
        .map_err(|err| anyhow::anyhow!(format!("{}", err.to_string())))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    env_logger::init();

    let database_url = dotenv::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // check if cardamon database has been built
    if !Path::new(&database_url).exists() {
        if fs::File::create(&database_url).is_err() {
            panic!("Error creating database file")
        }
    }

    // start sqlite connection and migrate the db
    let mut db_conn = SqliteConnection::establish(&database_url)?;
    run_migrations(&mut db_conn);

    let cli = Cli::parse();

    // run the scenarios
    match &cli.command {
        Commands::Scenario(args) => {
            let telegraf_conf_path = args.telegraf_conf.clone().unwrap_or("telegraf.conf".into());
            let scenarios_path = args.path.clone().unwrap_or("scenarios".into());

            match &args.command {
                ScenarioCommands::Run { scenario: None } => {
                    let cardamon_run_id =
                        init_scenario_run(Arc::new(Mutex::new(db_conn)), telegraf_conf_path)
                            .await?;

                    let mut scenarios_run: Vec<String> = vec![];
                    // Check for single file / directory input
                    if scenarios_path.is_dir() {
                        let dir_entries = fs::read_dir(scenarios_path)?;
                        for dir_entry in dir_entries {
                            let scenario_path = dir_entry?.path();
                            match scenario_runner::run(&scenario_path, &cardamon_run_id).await {
                                Ok(scenario_name) => scenarios_run.push(scenario_name.to_string()),
                                Err(_err) => {}
                            }
                        }
                    } else if scenarios_path.is_file() {
                        match scenario_runner::run(&scenarios_path, &cardamon_run_id).await {
                            Ok(scenario_name) => scenarios_run.push(scenario_name.to_string()),
                            Err(_err) => {}
                        }
                    } else {
                        eprintln!("{:?}, is not a valid directory or file", scenarios_path);
                    }

                    let summary = generate_scenario_summary(scenarios_run)?;
                    println!("{}", summary);
                }

                ScenarioCommands::Run {
                    scenario: Some(scenario),
                } => {
                    let cardamon_run_id =
                        init_scenario_run(Arc::new(Mutex::new(db_conn)), telegraf_conf_path)
                            .await?;

                    let scenario_path = scenarios_path.join(scenario);

                    match scenario_runner::run(&scenario_path, &cardamon_run_id).await {
                        Ok(_scenario_name) => {}
                        Err(_err) => {}
                    }
                }
            }
        }

        Commands::TelemetryServer(args) => match args.command {
            TelemetryServerCommands::Run { port } => {
                info!("running server on port {}", port);
            }
        },
    }

    Ok(())
}
