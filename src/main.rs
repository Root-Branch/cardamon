use anyhow::Context;
use cardamon::{
    cleanup_stdout_stderr,
    config::{self, Config, ExecutionPlan, ProcessToObserve},
    data::Data,
    db_connect, db_migrate, init_config,
    models::rab_linear_model,
    run, server,
};
use clap::{Parser, Subcommand};
use colored::Colorize;
use dotenvy::dotenv;
use itertools::Itertools;
use std::{env, path::Path};
use tracing::{trace, Level};

#[derive(Parser, Debug)]
#[command(author = "Oliver Winks (@ohuu), William Kimbell (@seal)", version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long)]
    pub file: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Runs a single observation")]
    Run {
        #[arg(help = "Please provide an observation name")]
        name: String,

        #[arg(value_name = "EXTERNAL PIDs", short, long, value_delimiter = ',')]
        pids: Option<Vec<String>>,

        #[arg(
            value_name = "EXTERNAL CONTAINER NAMES",
            short,
            long,
            value_delimiter = ','
        )]
        containers: Option<Vec<String>>,

        #[arg(long)]
        external_only: bool,
    },

    #[command(about = "Run continuously")]
    Live {
        #[arg(help = "Please provide a system name")]
        name: String,

        #[arg(value_name = "EXTERNAL PIDs", short, long, value_delimiter = ',')]
        pids: Option<Vec<String>>,

        #[arg(
            value_name = "EXTERNAL CONTAINER NAMES",
            short,
            long,
            value_delimiter = ','
        )]
        containers: Option<Vec<String>>,

        #[arg(long)]
        external_only: bool,
    },

    #[command(about = "Start the Cardamon UI server")]
    Ui {
        #[arg(short, long)]
        port: Option<u32>,
    },

    #[command(about = "Wizard for creating a cardamon.toml file")]
    Init,
}

fn load_config(file: &Option<String>) -> anyhow::Result<Config> {
    // Initialize config if it exists
    match file {
        Some(path) => {
            println!("> using config {}", path.green());
            config::Config::try_from_path(Path::new(path))
        }
        None => {
            println!("> using config {}", "./cardamon.toml".green());
            config::Config::try_from_path(Path::new("./cardamon.toml"))
        }
    }
}

fn add_external_processes(
    pids: Option<Vec<String>>,
    containers: Option<Vec<String>>,
    exec_plan: &mut ExecutionPlan,
) -> anyhow::Result<()> {
    // add external processes to observe.
    for pid in pids.unwrap_or_default() {
        let pid = pid.parse::<u32>()?;
        println!("> including external process {}", pid.to_string().green());
        exec_plan.observe_external_process(ProcessToObserve::ExternalPid(pid));
    }
    if let Some(container_names) = containers {
        exec_plan.observe_external_process(ProcessToObserve::ExternalContainers(container_names));
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // read .env file if it exists
    dotenv().ok();

    // Parse clap args
    let args = Cli::parse();

    // Set the debug level, prioritizing command-line args over config
    let log_level = match env::var("LOG_LEVEL").unwrap_or("WARN".to_string()).as_str() {
        "TRACE" => Level::TRACE,
        "DEBUG" => Level::DEBUG,
        "INFO" => Level::INFO,
        "WARN" => Level::WARN,
        "ERROR" => Level::ERROR,
        _ => Level::WARN,
    };

    // Set up tracing subscriber
    let subscriber = tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .pretty()
        .with_max_level(log_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    trace!("Setup subscriber for logging");

    // connect to the database and run migrations
    let database_url =
        env::var("DATABASE_URL").unwrap_or("sqlite://cardamon.db?mode=rwc".to_string());
    let database_name = env::var("DATABASE_NAME").unwrap_or("".to_string());
    let db_conn = db_connect(&database_url, Some(&database_name)).await?;
    db_migrate(&db_conn).await?;

    match args.command {
        Commands::Init => {
            init_config().await;
        }

        Commands::Run {
            name,
            pids,
            containers,
            external_only,
        } => {
            println!("\n{}", " Cardamon ".reversed().green());
            let config = load_config(&args.file)
                .context("Error loading configuration, please run `cardamon init`")?;

            // create an execution plan
            let mut execution_plan = config.create_execution_plan(&name, external_only)?;

            // add external processes to observe.
            add_external_processes(pids, containers, &mut execution_plan)?;

            // Cleanup previous runs stdout and stderr
            cleanup_stdout_stderr()?;

            // run it!
            let observation_dataset = run(execution_plan, config.cpu.avg_power, &db_conn).await?;

            println!("\n{}", " Summary ".reversed().green());
            for scenario_dataset in observation_dataset.by_scenario(false).iter() {
                let run_datasets = scenario_dataset.by_run();

                // execute model for current run
                let f = rab_linear_model(42.0);
                let (head, tail) = run_datasets
                    .split_first()
                    .expect("Dataset does not include recent run.");
                let run_data = head.apply_model(&db_conn, &f).await?;

                // execute model for previous runs and calculate trend
                let mut tail_data = vec![];
                for run_dataset in tail {
                    let run_data = run_dataset.apply_model(&db_conn, &f).await?;
                    tail_data.push(run_data.data);
                }
                let tail_data = Data::mean(&tail_data.iter().collect_vec());
                let trend = run_data.data.pow - tail_data.pow;
                let trend_str = if trend > 0.0 {
                    format!("↓ {:.2} W", trend).green()
                } else {
                    format!("↑ {:.2} W", trend.abs()).red()
                };

                println!(
                    "{}: Co2 {} | Power {} | Trend {}",
                    scenario_dataset.scenario_name(),
                    format!("{:.2} g", run_data.data.co2).green(),
                    format!("{:.2} W", run_data.data.pow).green(),
                    trend_str
                )
            }
            println!("\n{}", "trend compared to previous 3 runs".bright_black());
        }

        Commands::Live {
            name,
            pids,
            containers,
            external_only,
        } => {
            println!("\n{}", " Cardamon ".reversed().green());

            // Initialize config if it exists
            let config = load_config(&args.file)
                .context("Error loading configuration, please run `cardamon init`")?;

            // create an execution plan
            let mut execution_plan = config.create_execution_plan(&name, external_only)?;

            add_external_processes(pids, containers, &mut execution_plan)?;

            // Cleanup previous runs stdout and stderr
            cleanup_stdout_stderr()?;
        }

        Commands::Ui { port } => {
            let port = port.unwrap_or(1337);
            server::start(port, &db_conn).await?
        }
    }

    Ok(())
}
