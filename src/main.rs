use cardamon::{clap_args, scenario_runner, settings};

use diesel::{prelude::*, SqliteConnection};
use std::sync::{Arc, Mutex};
use tracing::{error, info};

use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenv::dotenv;

use core::panic;
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
        .unwrap_or_else(|_| {
            panic!("Failed to execute 'telegraf --version' command. Is Telegraf installed?")
        });

    // Parse clap args
    let args = clap_args::parse();
    println!("{:?}", args);
    // Log level, toml parsing and validation
    println!("Verbose mode: {}", args.verbose);
    //let settings = settings::Settings::parse(name, args.verbose);
    match args.command {
        clap_args::Commands::Run { name } => {
            println!("Running with config name {} ", name);
            let settings = settings::Settings::parse(name, args.verbose);
            // DB_URL is safe here, validated in settings
            let database_url = settings.config.database_url.clone().unwrap();
            if !Path::new(&database_url).exists() {
                if fs::File::create(&database_url).is_err() {
                    panic!("Error creating database file")
                }
            }
            // start sqlite connection and migrate the db
            let mut db_conn = SqliteConnection::establish(&database_url)
                .expect("failed to establish db connection");
            run_migrations(&mut db_conn);

            let shared_db_conn = Arc::new(Mutex::new(db_conn));

            //scenario_runner::start_scenarios(&settings, shared_db_conn);
            match scenario_runner::start_scenarios(&settings, shared_db_conn).await {
                Ok(..) => info!("Started and ran scenarios successfully"),
                Err(e) => error!("Error running scenarios {e}"),
            }

            //c;w
            //println!("{:?}", settings);
        }
    }
    Ok(())
}
fn run_migrations(db_conn: &mut impl MigrationHarness<DB>) {
    db_conn
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to migrate the cardamon database!");
}
