use cardamon::{clap_args, scenario_runner, settings};

use anyhow::Context;
use diesel::{prelude::*, SqliteConnection};
use std::sync::{Arc, Mutex};
use tracing::{error, info};

use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenv::dotenv;

use std::{fs, path::Path, process::Command};
type DB = diesel::sqlite::Sqlite;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // dotenv
    dotenv().ok();
    // DB_URL will be over-ridden in config parse if set
    env_logger::init();

    //Ensure telegraf is installed
    let _ = Command::new("telegraf")
        .arg("--version")
        .output()
        .context("Failed to execute 'telegraf --version'")?;

    // Parse clap args
    let args = clap_args::parse();
    println!("{:?}", args);
    // Log level, toml parsing and validation
    println!("Verbose mode: {}", args.verbose);
    match args.command {
        clap_args::Commands::Run { name } => {
            println!("Running with config name {} ", name);
            let settings =
                settings::parse(name, args.verbose).context("Failed to parse settings:")?;

            // DB_URL is safe here, validated in settings
            //let database_url = settings.database_url.clone();
            if !Path::new(&settings.database_url).exists() {
                fs::File::create(&settings.database_url).with_context(|| {
                    format!("failed to create database file {} ", settings.database_url)
                })?;
            }
            // start sqlite connection and migrate the db
            let mut db_conn =
                SqliteConnection::establish(&settings.database_url).with_context(|| {
                    format!(
                        "Failed to establish sqlite connection to : {}",
                        settings.database_url
                    )
                })?;
            run_migrations(&mut db_conn);

            let shared_db_conn = Arc::new(Mutex::new(db_conn));

            match scenario_runner::start_scenarios(&settings, shared_db_conn).await {
                Ok(..) => info!("Started and ran scenarios successfully"),
                Err(e) => error!("Error running scenarios {e}"),
            }
        }
    }
    Ok(())
}
fn run_migrations(db_conn: &mut impl MigrationHarness<DB>) {
    db_conn
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to migrate the cardamon database!");
}
