use cardamon::{config, data_access::LocalDataAccessService, run};
use tracing::{subscriber::set_global_default, Subscriber};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt::writer::MakeWriterExt, layer::SubscriberExt, EnvFilter, Registry};

use clap::{Parser, Subcommand};
use sqlx::{migrate::MigrateDatabase, SqlitePool};
use std::fs::File;

#[derive(Parser, Debug)]
#[command(author = "Oliver Winks (@ohuu), William Kimbell (@seal)", version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, action = clap::ArgAction::SetFalse)]
    pub verbose: bool,

    #[arg(short, long)]
    pub file: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Run { name: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse clap args
    let args = Cli::parse();

    match args.command {
        Commands::Run { name } => {
            // set up local data access
            let pool = create_db().await?;
            let data_access_service = LocalDataAccessService::new(pool);

            let config = config::Config::from_path(std::path::Path::new("./cardamon.toml"))?;
            init_subscriber(get_subscriber(
                "cardamon".into(),
                config.debug_level.clone().unwrap_or("info".to_string()),
            ));
            // create an execution plan
            let execution_plan = config.create_execution_plan(&name)?;

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
fn get_subscriber(name: String, env_filter: String) -> impl Subscriber + Sync + Send {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));

    let file_writer = File::create("debug.log").unwrap();
    let stdout_writer = std::io::stdout;
    let formatting_layer = BunyanFormattingLayer::new(name, file_writer.and(stdout_writer));

    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
}
fn init_subscriber(subscriber: impl Subscriber + Sync + Send) {
    LogTracer::init().expect("Failed to set logger");
    set_global_default(subscriber).expect("Failed to set subscriber");
}
