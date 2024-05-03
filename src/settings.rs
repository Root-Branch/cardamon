use core::panic;

use std::{fs, path::PathBuf};
use tracing::{info, Level};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub config: Config,
    pub global_settings: GlobalSettings,
}
pub fn parse(config_name: String, verbose: bool) -> Settings {
    Settings::parse(config_name, verbose)
}
#[derive(Debug, Deserialize)]
pub struct GlobalSettings {
    pub debug_level: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct Config {
    mode: String,
    pub database_url: Option<String>,
    pub runs: Option<Vec<String>>,
    commands: Option<Vec<String>>,
    command: Option<String>,
    directory: Option<String>,
}
impl Settings {
    pub fn parse(config_name: String, verbose: bool) -> Self {
        // Set DB URL based upon config

        let file = fs::read_to_string("cardamon.toml").expect("Failed to read cardamon.conf");
        let mut settings: Settings = toml::from_str(&file).expect("Failed to parse cardamon.conf");

        // If in config, get db
        // Else check env, if it's there, set the settings.config.db_url to it
        // Else panic ?
        match settings.config.database_url.clone() {
            Some(db_url) => std::env::set_var("DATABASE_URL", db_url),
            None => {
                settings.config.database_url = Some(dotenv::var("DATABASE_URL").expect(
                    "DATABASE_URL must be set in config or env var (Config takes priority)",
                ));
            }
        }
        // Convert the debug_level string to the corresponding Level value
        let mut debug_level = Level::INFO;
        if !verbose {
            // Otherwise, level is set to "INFO" above
            debug_level = match settings.global_settings.debug_level {
                Some(ref level) => match level.to_lowercase().as_str() {
                    "trace" => Level::TRACE,
                    "debug" => Level::DEBUG,
                    "info" => Level::INFO,
                    "warn" => Level::WARN,
                    "error" => Level::ERROR,
                    _ => {
                        eprintln!(
                            "Error with config debug level: {}, setting debug level to \"error\"",
                            level
                        );
                        Level::ERROR
                    }
                },
                None => Level::INFO,
            };
        }

        // Set the debug level using the tracing subscriber
        tracing::subscriber::set_global_default(
            tracing_subscriber::fmt()
                .with_max_level(debug_level)
                .finish(),
        )
        .expect("Failed to set global default subscriber");
        info!("Set global default subscriber");
        match settings.config.mode.as_str() {
            "default" => {
                match settings.config.directory {
                    Some(..) => {
                        panic!("Cannot use directory in default mode");
                    }
                    None => (),
                }
                settings.config.runs = settings.config.commands.clone();
            }
            "dir" => {
                // Path to scenarios
                let command = match settings.config.command {
                    Some(ref cmd) => cmd.clone(),
                    None => {
                        panic!("Please provide a command (containing {{file}}) to use dir mode");
                    }
                };
                let directory = match settings.config.directory.clone() {
                    Some(dir) => PathBuf::from(dir),
                    None => panic!("Please provide a directory in config to use dir mode"),
                };
                if !directory.is_dir() {
                    panic!(
                        "{} (config scenario dir) is an invalid directory",
                        directory.to_string_lossy()
                    );
                }
                let mut runs = Vec::new();
                for entry in
                    fs::read_dir(directory).expect("Failed to read configuration directory")
                {
                    let entry = entry.expect("Failed to read directory entry");
                    let path = entry.path();
                    if path.is_file() {
                        let file_path = path.to_string_lossy().to_string();
                        let command = command.replace("{file}", &file_path);
                        runs.push(command);
                    }
                }
                settings.config.runs = Some(runs);
            }
            _ => {
                // do stuff
            }
        }
        settings
    }
}
