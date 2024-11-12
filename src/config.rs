use anyhow::Context;
use colored::Colorize;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};
use sysinfo::{CpuRefreshKind, RefreshKind, System};

#[cfg(not(windows))]
static EXAMPLE_CONFIG: &str = include_str!("templates/cardamon.unix.toml");
#[cfg(windows)]
static EXAMPLE_CONFIG: &str = include_str!("templates/cardamon.win.toml");

#[cfg(not(windows))]
static LINE_ENDING: &str = "\n";
#[cfg(windows)]
static LINE_ENDING: &str = "\r\n";

// ******** ******** ********
// **    CONFIGURATION     **
// ******** ******** ********
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub cpu: Cpu,
    #[serde(rename(serialize = "process", deserialize = "process"))]
    pub processes: Vec<Process>,
    #[serde(rename(serialize = "scenario", deserialize = "scenario"))]
    pub scenarios: Vec<Scenario>,
    #[serde(rename(serialize = "observation", deserialize = "observation"))]
    pub observations: Vec<Observation>,
}
impl Config {
    pub fn write_example_to_file(
        cpu_name: &str,
        cpu_power: Power,
        path: &std::path::Path,
    ) -> anyhow::Result<File> {
        // remove the line containing tdp
        let mut lines = EXAMPLE_CONFIG.lines().map(|s| s.to_string()).collect_vec();

        // add a line at the top of the file containing the new tdp
        let mut new_conf_lines = match cpu_power {
            Power::Tdp(tdp) => vec![
                "[cpu]".to_string(),
                format!("name = \"{}\"", cpu_name),
                format!("tdp = {}", tdp),
                "".to_string(),
            ],

            Power::Curve(a, b, c, d) => vec![
                "[cpu]".to_string(),
                format!("name = \"{}\"", cpu_name),
                format!("curve = [{},{},{},{}]", a, b, c, d),
                "".to_string(),
            ],
        };

        new_conf_lines.append(&mut lines);
        let conf_str = new_conf_lines.join(LINE_ENDING);

        // write to file
        let mut file = File::create_new(path)?;
        File::write_all(&mut file, conf_str.as_bytes())?;
        Ok(file)
    }

    pub fn try_from_path(path: &std::path::Path) -> anyhow::Result<Config> {
        let mut config_str = String::new();
        fs::File::open(path)?.read_to_string(&mut config_str)?;
        Config::try_from_str(&config_str)
    }

    pub fn try_from_str(conf_str: &str) -> anyhow::Result<Config> {
        toml::from_str::<Config>(conf_str).map_err(|e| anyhow::anyhow!("TOML parsing error: {}", e))
    }

    pub fn find_observation(&self, obs_name: &str) -> Option<&Observation> {
        self.observations.iter().find(|obs| match obs {
            Observation::LiveMonitor { name, processes: _ } => name == obs_name,
            Observation::ScenarioRunner { name, scenarios: _ } => name == obs_name,
        })
    }

    pub fn find_scenario(&self, scenario_name: &str) -> anyhow::Result<&Scenario> {
        self.scenarios
            .iter()
            .find(|scenario| scenario.name == scenario_name)
            .context(format!(
                "Unable to find scenario with name {}",
                scenario_name
            ))
    }

    pub fn find_scenarios(&self, scenario_names: &[&String]) -> anyhow::Result<Vec<&Scenario>> {
        let mut scenarios = vec![];
        for scenario_name in scenario_names {
            let scenario = self.find_scenario(scenario_name)?;
            scenarios.push(scenario);
        }
        Ok(scenarios)
    }

    /// Finds a process in the config with the given name.
    ///
    /// # Arguments
    /// * proc_name - the name of the process to find
    ///
    /// # Returns
    /// Some process if it can be found, None otherwise
    fn find_process(&self, proc_name: &str) -> anyhow::Result<&Process> {
        self.processes
            .iter()
            .find(|proc| proc.name == proc_name)
            .context(format!("Unable to find process with name {}", proc_name))
    }

    pub fn find_processes(&self, proc_names: &[&String]) -> anyhow::Result<Vec<&Process>> {
        let mut processes = vec![];
        for proc_name in proc_names {
            let proc = self.find_process(proc_name)?;
            processes.push(proc);
        }
        Ok(processes)
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Power {
    Curve(f64, f64, f64, f64),
    Tdp(f64),
}

#[derive(Debug, Deserialize, PartialEq, Serialize, Clone)]
pub struct Cpu {
    pub name: String,
    #[serde(flatten)]
    pub power: Power,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy, Serialize)]
#[serde(tag = "to", rename_all = "lowercase")]
pub enum Redirect {
    Null,
    Parent,
    File,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ProcessType {
    BareMetal,
    Docker { containers: Vec<String> },
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Process {
    pub name: String,
    pub up: String,
    pub down: Option<String>,
    pub redirect: Option<Redirect>,
    #[serde(rename = "process")]
    pub process_type: ProcessType,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Scenario {
    pub name: String,
    pub desc: String,
    pub command: String,
    pub iterations: i32,
    pub processes: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Observation {
    LiveMonitor {
        name: String,
        processes: Vec<String>,
    },
    ScenarioRunner {
        name: String,
        scenarios: Vec<String>,
    },
}

fn ask_for_tdp() -> Power {
    loop {
        print!("Please enter the TDP of your CPU in watts: ");
        let _ = std::io::stdout().flush();

        let mut input = String::new();
        let res = std::io::stdin().read_line(&mut input);
        match res {
            Ok(_) => match input.trim().parse::<f64>() {
                Ok(parsed_input) => {
                    return Power::Tdp(parsed_input);
                }
                Err(_) => {
                    println!("{}", "Please enter a valid number.".yellow());
                    continue;
                }
            },
            Err(_) => continue,
        }
    }
}

fn ask_for_cpu() -> String {
    loop {
        print!("Please enter a CPU name: ");
        let _ = std::io::stdout().flush();

        let mut input = String::new();
        let res = std::io::stdin().read_line(&mut input);
        match res {
            Ok(_) => return input,
            Err(_) => continue,
        }
    }
}

fn find_cpu() -> Option<String> {
    let sys = System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::everything()));
    sys.cpus().first().map(|cpu| cpu.brand().to_string())
}

fn try_power_curve(json_obj: &Value) -> Option<Power> {
    let params_obj = json_obj.get("verbose")?.get("params")?.get("value")?;

    let a = params_obj.get("a")?.as_f64()?;
    let b = params_obj.get("b")?.as_f64()?;
    let c = params_obj.get("c")?.as_f64()?;
    let d = params_obj.get("d")?.as_f64()?;

    Some(Power::Curve(a, b, c, d))
}

fn try_tdp(json_obj: &Value) -> Option<Power> {
    let tdp = json_obj
        .get("verbose")?
        .get("tdp")?
        .get("value")?
        .as_f64()?;

    Some(Power::Tdp(tdp))
}

async fn fetch_power(cpu_name: &str) -> anyhow::Result<Power> {
    let client = reqwest::Client::new();
    let mut json = HashMap::new();
    json.insert("name", cpu_name);

    let resp = client
        .post("https://api.boavizta.org/v1/component/cpu")
        .header("Content-Type", "application/json")
        .json(&json)
        .send()
        .await?;

    let json_obj = resp.json().await?;

    try_power_curve(&json_obj)
        .or(try_tdp(&json_obj))
        .context("Error fetching power from Boavizta!")
}

/// Attempts to find the users CPU automatically and asks the user to enter it manually if that
/// fails.
pub async fn init_config() {
    let cpu_name: String;

    println!("\n{}", " Setting up Cardamon ".reversed().green());
    loop {
        print!("Would you like to create a config for this computer [1] or another computer [2]? ");
        let _ = std::io::stdout().flush();

        let mut ans = String::new();
        let res = std::io::stdin().read_line(&mut ans);
        match res {
            Ok(_) => {
                let opt = ans.trim().parse::<u32>();
                match opt {
                    Ok(1) => {
                        cpu_name = match find_cpu() {
                            Some(name) => {
                                println!("{} {}", "It looks like you have a".yellow(), name);
                                name
                            }
                            None => {
                                println!("{}", "Unable to find CPU!".red());
                                ask_for_cpu()
                            }
                        };
                        break;
                    }
                    Ok(2) => {
                        cpu_name = ask_for_cpu();
                        break;
                    }
                    _ => {
                        println!("{}", "Please enter 1 or 2.\n".yellow());
                        continue;
                    }
                }
            }
            Err(_) => {
                println!("{}", "Please enter 1 or 2.\n".yellow());
                continue;
            }
        }
    }

    let power = match fetch_power(&cpu_name).await {
        Ok(pow @ Power::Curve(a, b, c, d)) => {
            let peak_pow = a * (b * (100.0 + c)).ln() + d;
            println!(
                "{} {}",
                "Boavista reports a peak power of".yellow(),
                peak_pow
            );
            pow
        }

        Ok(pow @ Power::Tdp(tdp)) => {
            println!("{} {}", "Boavizta reports a tdp of".yellow(), tdp);
            pow
        }

        Err(_) => {
            println!("{}", "Cannot get power from Boavizta for your CPU!".red());
            ask_for_tdp()
        }
    };

    match Config::write_example_to_file(&cpu_name, power, Path::new("./cardamon.toml")) {
        Ok(_) => {
            println!("{}", "cardamon.toml created!".green());
            println!("\n🤩\n");
        }

        Err(err) => {
            println!("{}\n{}", "Error creating config.".red(), err);
            println!("\n😭\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn should_find_cpu() {
        let cpu_name = find_cpu();
        assert!(cpu_name.is_some())
    }

    #[tokio::test]
    async fn fetch_tdp_should_work() -> anyhow::Result<()> {
        let cpu_name = find_cpu();

        if let Some(cpu_name) = cpu_name {
            let power = fetch_power(&cpu_name).await?;
            match power {
                Power::Curve(_, _, _, _) => assert!(true),
                Power::Tdp(tdp) => assert!(tdp > 0.0),
            }
            return Ok(());
        }

        panic!()
    }

    #[test]
    fn can_load_config_file() -> anyhow::Result<()> {
        Config::try_from_path(Path::new("./fixtures/cardamon.success.toml"))?;
        Ok(())
    }

    #[test]
    fn can_find_observation_by_name() -> anyhow::Result<()> {
        let cfg = Config::try_from_path(Path::new("./fixtures/cardamon.success.toml"))?;
        let observation = cfg.find_observation("checkout");
        assert!(observation.is_some());

        let observation = cfg.find_observation("nope");
        assert!(observation.is_none());

        Ok(())
    }

    #[test]
    fn can_find_scenario_by_name() -> anyhow::Result<()> {
        let cfg = Config::try_from_path(Path::new("./fixtures/cardamon.multiple_scenarios.toml"))?;
        let scenario = cfg.find_scenario("user_signup");
        assert!(scenario.is_ok());

        let scenario = cfg.find_scenario("nope");
        assert!(scenario.is_err());

        Ok(())
    }

    #[test]
    fn can_find_process_by_name() -> anyhow::Result<()> {
        let cfg = Config::try_from_path(Path::new("./fixtures/cardamon.success.toml"))?;
        let process = cfg.find_process("server");
        assert!(process.is_ok());

        let process = cfg.find_process("nope");
        assert!(process.is_err());

        Ok(())
    }
}
