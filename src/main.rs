use std::path::Path;

use cardamon::{
    config::{self, ProcessToObserve},
    data_access::LocalDataAccessService,
    init_config, run,
};
use clap::{Parser, Subcommand};
use sqlx::{migrate::MigrateDatabase, SqlitePool};
use tracing::Level;

#[derive(Parser, Debug)]
#[command(author = "Oliver Winks (@ohuu), William Kimbell (@seal)", version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long)]
    pub verbose: bool,

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
    Init,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse clap args
    let args = Cli::parse();

    // Set the debug level, prioritizing command-line args over config
    let level = match args.verbose {
        true => Level::DEBUG,
        false => Level::INFO,
    };

    // Set up tracing subscriber
    let subscriber = tracing_subscriber::fmt().with_max_level(level).finish();
    tracing::subscriber::set_global_default(subscriber)?;

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
            };
            let config = match config {
                Ok(cfg) => Some(cfg),
                Err(e) => {
                    eprintln!(
                        "Error loading configuration, please run `cardamon init`: {}",
                        e
                    );
                    None
                }
            };

            // Ensure we have a config for the Run command
            let config = match config {
                Some(cfg) => cfg,
                None => return Err(anyhow::anyhow!("No config file found for Run command")),
            };

            // set up local data access
            let pool = create_db().await?;
            let data_access_service = LocalDataAccessService::new(pool);

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
            // run it!
            let observation_dataset = run(execution_plan, &data_access_service).await?;

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
    }

    Ok(())
}
async fn create_db() -> anyhow::Result<SqlitePool> {
    let db_url = "sqlite://cardamon.db";
    if !sqlx::Sqlite::database_exists(db_url).await? {
        sqlx::Sqlite::create_database(db_url).await?;
    }

    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename("cardamon.db")
                .pragma("journal_mode", "DELETE"), // Disable WAL mode
        )
        // .connect(db_url) with wal and shm
        .await?;

    sqlx::migrate!().run(&db).await?;

    Ok(db)
}
