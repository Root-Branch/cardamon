use core::panic;
use std::{fs, path::PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub config: Config,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    mode: String,
    pub runs: Option<Vec<String>>,
    commands: Option<Vec<String>>,
    command: Option<String>,
    directory: Option<String>,
}
impl Settings {
    pub fn parse() -> Self {
        let file = fs::read_to_string("cardamon.toml").expect("Failed to read cardamon.conf");
        let mut settings: Settings = toml::from_str(&file).expect("Failed to parse cardamon.conf");
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
