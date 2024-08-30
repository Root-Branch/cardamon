use anyhow::Context;
use cardamon::{
    cleanup_stdout_stderr,
    config::{self, ProcessToObserve},
    init_config,
    migrations::{Migrator, MigratorTrait},
    run, server,
};
use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
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
    Run {
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
    Ui {
        #[arg(short, long)]
        port: Option<u32>,
    },
    Init,
}

async fn db_connect() -> anyhow::Result<DatabaseConnection> {
    let database_url =
        &env::var("DATABASE_URL").unwrap_or("sqlite://cardamon.db?mode=rwc".to_string());
    let database_name = &env::var("DATABASE_NAME").unwrap_or("".to_string());

    let db = Database::connect(database_url).await?;
    match db.get_database_backend() {
        DbBackend::Sqlite => Ok(db),

        DbBackend::Postgres => {
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

async fn db_migrate(db_conn: &DatabaseConnection) -> anyhow::Result<()> {
    Migrator::up(db_conn, None)
        .await
        .context("Error migrating database.")
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
        .pretty()
        .with_max_level(log_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    trace!("Setup subscriber for logging");

    // connect to the database and run migrations
    let db_conn = db_connect().await?;
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
            // Initialize config if it exists
            let config = match &args.file {
                Some(path) => config::Config::try_from_path(Path::new(path)),
                None => config::Config::try_from_path(Path::new("./cardamon.toml")),
            }
            .context("Error loading configuration, please run `cardamon init`")?;

            // create an execution plan
            let mut execution_plan = if external_only {
                config.create_execution_plan_external_only(&name)
            } else {
                config.create_execution_plan(&name)
            }?;

            // add external processes to observe.
            for pid in pids.unwrap_or_default() {
                let pid = pid.parse::<u32>()?;
                execution_plan.observe_external_process(ProcessToObserve::Pid(None, pid));
            }
            for container_name in containers.unwrap_or_default() {
                execution_plan
                    .observe_external_process(ProcessToObserve::ContainerName(container_name));
            }
            // Cleanup previous runs stdout and stderr
            cleanup_stdout_stderr()?;

            // run it!
            let observation_dataset = run(execution_plan, &db_conn).await?;

            for scenario_dataset in observation_dataset.by_scenario().iter() {
                println!("Scenario: {:?}", scenario_dataset.scenario_name());
                println!("--------------------------------");

                for run_dataset in scenario_dataset.by_run().iter() {
                    println!("Run: {:?}", run_dataset.run_id());

                    for avged_dataset in run_dataset.averaged().iter() {
                        println!("\t{:?}", avged_dataset);
                    }
                }
            }
        }

        Commands::Ui { port } => {
            let port = port.unwrap_or(1337);
            server::start(port, &db_conn).await?
        }
    }

    Ok(())
}
