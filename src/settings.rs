use std::error::Error;
use std::{fmt, io};
use std::{fs, path::PathBuf};
use toml::de::Error as TomlError;
use tracing::subscriber::SetGlobalDefaultError;
use tracing::{info, Level};

use serde::Deserialize;

// Custom enum errors, prevents massive repition of :
/*
 * .map_err(| e | format!....
 * Can just do
 * map_err(|e| ParseError::TomlError(e))?;
 *
 */
#[derive(Debug)]
pub enum ParseError {
    IoError(io::Error),
    TomlError(TomlError),
    InvalidDebugLevel(String),
    MissingCommand,
    InvalidDirectory(String),
    InvalidEntry(String),
    ConfigNotFound(String),
    SubscriberError(SetGlobalDefaultError),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::IoError(e) => write!(f, "I/O error: {}", e),
            ParseError::TomlError(e) => write!(f, "TOML parsing error: {}", e),
            ParseError::InvalidDebugLevel(level) => write!(f, "Invalid debug level: {}", level),
            ParseError::MissingCommand => write!(f, "Missing command for directory mode"),
            ParseError::InvalidDirectory(dir) => write!(f, "Invalid directory: {}", dir),
            ParseError::InvalidEntry(entry) => write!(f, "Invalid entry: {}", entry),
            ParseError::ConfigNotFound(name) => write!(f, "Config not found: {}", name),
            ParseError::SubscriberError(sub_error) => {
                write!(f, "Setting default subscriber error: {}", sub_error)
            }
        }
    }
}
impl From<SetGlobalDefaultError> for ParseError {
    fn from(err: SetGlobalDefaultError) -> ParseError {
        ParseError::SubscriberError(err)
    }
}
impl From<io::Error> for ParseError {
    fn from(err: io::Error) -> ParseError {
        ParseError::IoError(err)
    }
}

impl From<TomlError> for ParseError {
    fn from(err: TomlError) -> ParseError {
        ParseError::TomlError(err)
    }
}
// This allows the parse error to be converting into a std::error::Error;
// Reason being, the main.rs Result returns a std::error::Error via an anyhow result
impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ParseError::IoError(e) => Some(e),
            ParseError::TomlError(e) => Some(e),
            _ => None,
        }
    }
}
pub fn parse(config_name: String, verbose: bool) -> Result<MainConfig, ParseError> {
    Settings::parse(config_name, verbose)
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    global: GlobalSettings,
    #[serde(rename = "config")] // Rename for ease of config use
    configs: Vec<Config>, // Called config in cardamon.toml,
}

#[derive(Debug, Deserialize)]
struct GlobalSettings {
    debug_level: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Config {
    name: String,
    database_url: String,
    #[serde(rename = "scenario")]
    scenarios: Vec<ConfigScenario>, // Called scenario in cardamon.toml
}

// This is the scenario for cardamon.toml
#[derive(Debug, Deserialize)]
struct ConfigScenario {
    name: Option<String>,
    iterations: Option<u32>,
    commands: Option<Vec<String>>,
    directory: Option<String>,
    command: Option<String>,
}
// to prevent issues with iterations, incorrect config "names" being passed and more
// Passing only the neccesary fields to the main.rs will be neccesary
// Here we pass database_url and "commands" which should be all that's neccesary
pub struct MainConfig {
    pub database_url: String,
    pub scenarios: Vec<Scenario>,
}
pub struct Scenario {
    pub name: String,
    pub iteration: u32, // A scenario with two iterations will have two structs pushed
    // One with iteration = 1, and one with iteration = 2
    // No need for main.rs / scenario_runner.rs to loop over
    pub command: String,
}
impl Settings {
    //pub fn parse(config_name: String, verbose: bool) -> Result<MainConfig, String> {
    pub fn parse(config_name: String, verbose: bool) -> Result<MainConfig, ParseError> {
        let file = fs::read_to_string("cardamon.toml").map_err(|e| ParseError::IoError(e))?;
        let settings: Settings = toml::from_str(&file).map_err(|e| ParseError::TomlError(e))?;

        // Convert the debug_level string to the corresponding Level value
        let mut debug_level = Level::INFO;
        if !verbose {
            debug_level = match settings.global.debug_level {
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
        )?;
        info!("Set global default subscriber");

        // Find the config with the specified name

        if let Some(config) = settings.configs.iter().find(|c| c.name == config_name) {
            let mut scenarios = Vec::new();

            for config_scenario in &config.scenarios {
                match config_scenario.directory {
                    Some(ref directory) => {
                        // Directory mode ( {{file}} )
                        let command = config_scenario
                            .command
                            .as_ref()
                            .ok_or_else(|| ParseError::MissingCommand)?;
                        // Path
                        let directory_path = PathBuf::from(directory);
                        // We want a directory, as we want to run the command for each file in the
                        // dir
                        if !directory_path.is_dir() {
                            return Err(ParseError::InvalidDirectory(
                                directory_path.to_string_lossy().to_string(),
                            ));
                        }
                        for entry in fs::read_dir(&directory_path)
                            .map_err(|e| ParseError::InvalidDirectory(e.to_string()))?
                        {
                            let entry =
                                entry.map_err(|e| ParseError::InvalidEntry(e.to_string()))?;
                            let path = entry.path();
                            if path.is_file() {
                                let file_path = path.to_string_lossy().to_string();
                                let command_with_file = command.replace("{file}", &file_path);
                                // Get iterations, default to 1
                                let num_iterations = config_scenario.iterations.unwrap_or(1);
                                for iteration in 1..=num_iterations {
                                    // Optional name, will default to commmand name if not
                                    // specified
                                    let name = config_scenario
                                        .name
                                        .clone()
                                        .unwrap_or(command_with_file.clone());
                                    scenarios.push(Scenario {
                                        name,
                                        iteration,
                                        command: command_with_file.clone(),
                                    });
                                }
                            }
                        }
                    }
                    None => {
                        // Default mode
                        if let Some(ref scenario_commands) = config_scenario.commands {
                            for (iteration, command) in scenario_commands.iter().enumerate() {
                                // Get iterations, default to 1
                                let num_iterations = config_scenario.iterations.unwrap_or(1);
                                for _ in 1..=num_iterations {
                                    let name =
                                        config_scenario.name.clone().unwrap_or(command.clone());
                                    scenarios.push(Scenario {
                                        name,
                                        iteration: (iteration + 1) as u32,
                                        command: command.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            Ok(MainConfig {
                database_url: config.database_url.clone(),
                scenarios,
            })
        } else {
            Err(ParseError::ConfigNotFound(config_name))
        }
    }
}
