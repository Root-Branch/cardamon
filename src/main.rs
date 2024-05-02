/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use cardamon::{metrics_server, scenario_runner, settings, telegraf};
use clap::{command, Args, Parser, Subcommand};
use core::panic;
use diesel::{prelude::*, SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenv::dotenv;
use nanoid::nanoid;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

use cardamon::metrics_server::dto;

type DB = diesel::sqlite::Sqlite;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

#[derive(Parser)]
#[command(author = "Oliver Winks (@ohuu), William Kimbell (@seal)", version, about, long_about = None)]
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
    //#[arg(long, short)]
    //path: Option<PathBuf>,

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
        String::from("http://127.0.0.1:2050"),
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
    let log_level = std::env::var("LEVEL").unwrap_or_else(|_| "info".to_string());
    let level = match log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };
    let subscriber = FmtSubscriber::builder().with_max_level(level).finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let settings = settings::Settings::parse();
    info!("Parsed settings with commands{:?}", settings);
    // Ensure telegrapf is installed
    let _ = Command::new("telegraf")
        .arg("--version")
        .output()
        .unwrap_or_else(|_| {
            panic!("Failed to execute 'telegraf --version' command. Is Telegraf installed?")
        });
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
            match &args.command {
                // No scenario given
                ScenarioCommands::Run { scenario: None } => {
                    // Metrics and telegraf
                    let cardamon_run_id =
                        init_scenario_run(Arc::new(Mutex::new(db_conn)), telegraf_conf_path)
                            .await?;

                    // For each command in config, run command
                    let mut scenarios_run: Vec<String> = vec![];
                    match settings.config.runs {
                        Some(ref runs) => {
                            for file_path in runs {
                                match scenario_runner::run(file_path, &cardamon_run_id).await {
                                    Ok(scenario_name) => {
                                        scenarios_run.push(scenario_name.to_string())
                                    }
                                    Err(e) => error!("Error with scenario {e}"),
                                }
                            }
                        }
                        None => {
                            panic!("No runs provided");
                        }
                    }
                    let summary = generate_scenario_summary(scenarios_run)?;
                    info!("{}", summary);
                }
                ScenarioCommands::Run {
                    scenario: Some(scenario),
                } => {
                    let cardamon_run_id =
                        init_scenario_run(Arc::new(Mutex::new(db_conn)), telegraf_conf_path)
                            .await?;
                    match scenario_runner::run(scenario, &cardamon_run_id).await {
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
