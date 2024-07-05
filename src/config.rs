/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use anyhow::Context;
use regex::Regex;
use reqwest;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
};
use sysinfo::{CpuRefreshKind, RefreshKind, System};
use url::form_urlencoded;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub debug_level: Option<String>,
    pub metrics_server_url: Option<String>,
    pub processes: Vec<ProcessToExecute>,
    pub scenarios: Vec<Scenario>,
    pub observations: Vec<Observation>,
    pub tdp: Option<u32>, // Not actually optional, will calculate TDP if no TDP is set
}
impl Config {
    pub fn from_path(path: &std::path::Path) -> anyhow::Result<Config> {
        let mut config_str = String::new();
        fs::File::open(path)?.read_to_string(&mut config_str)?;
        // Not verbose, gives "Error parsing config file" and no more context
        //toml::from_str::<Config>(&config_str).context("Error parsing config file.")
        toml::from_str::<Config>(&config_str)
            .map_err(|e| anyhow::anyhow!("TOML parsing error: {}", e))
    }
    pub fn with_tdp(&mut self, tdp: u32) -> &mut Self {
        self.tdp = Some(tdp);
        self
    }
    pub fn config_to_file(path: &std::path::Path, config: Config) -> anyhow::Result<()> {
        // Do *not* need to check if path exists, create_new will fail if file exists
        let mut file = File::create_new(path)?;
        let toml_string = toml::to_string(&config)?;
        file.write_all(toml_string.as_bytes())?;
        Ok(())
    }
    #[cfg(unix)]
    pub fn default() -> Self {
        Config {
            debug_level: Some("info".to_string()),
            metrics_server_url: None,
            processes: vec![
                ProcessToExecute {
                    name: "test".to_string(),
                    up: "bash -c \"while true; do shuf -i 0-1337 -n 1; done\"".to_string(),
                    down: Some("kill {pid}".to_string()),
                    redirect: Some(Redirect::File),
                    process: ProcessType::BareMetal,
                },
                ProcessToExecute {
                    name: "test".to_string(),
                    up: "bash -c \"while true; do shuf -i 0-1337 -n 1; done\"".to_string(),
                    down: Some("kill {pid}".to_string()),
                    redirect: Some(Redirect::File),
                    process: ProcessType::Docker {
                        containers: vec![
                            "container_name_1".to_string(),
                            "container_name_2".to_string(),
                        ],
                    },
                },
            ],
            scenarios: vec![Scenario {
                name: "basket_10".to_string(),
                desc: "add 10 items to the basket".to_string(),
                command: "sleep 15".to_string(),
                iterations: 2,
                processes: vec!["test".to_string()],
            }],
            observations: vec![Observation {
                name: "obs_1".to_string(),
                scenarios: vec!["basket_10".to_string()],
            }],
            tdp: None,
        }
    }
    #[cfg(windows)]
    pub fn default() -> Self {
        Config {
            debug_level: Some("info".to_string()),
            metrics_server_url: None,
            processes: vec![ProcessToExecute {
                name: "test".to_string(),
                up: "powershell while($true) { get-random}".to_string(),
                down: Some("stop-process {pid}".to_string()),
                redirect: Some(Redirect::Null),
                process: ProcessType::BareMetal,
            }],
            scenarios: vec![Scenario {
                name: "basket_10".to_string(),
                desc: "Adds ten items to the basket".to_string(),
                command: "powershell sleep 15".to_string(),
                iterations: 2,
                processes: vec!["test".to_string()],
            }],
            observations: vec![Observation {
                name: "obs_1".to_string(),
                scenarios: vec!["basket_10".to_string()],
            }],
            tdp: None,
        }
    }

    fn find_observation(&self, observation_name: &str) -> Option<&Observation> {
        self.observations
            .iter()
            .find(|obs| obs.name == observation_name)
    }

    fn find_scenario(&self, scenario_name: &str) -> Option<&Scenario> {
        self.scenarios
            .iter()
            .find(|scenario| scenario.name == scenario_name)
    }

    /// Finds a process in the config with the given name.
    ///
    /// # Arguments
    /// * proc_name - the name of the process to find
    ///
    /// # Returns
    /// Some process if it can be found, None otherwise
    fn find_process(&self, proc_name: &str) -> Option<&ProcessToExecute> {
        self.processes.iter().find(|proc| proc.name == proc_name)
    }

    /// Finds the intersection of processes across all the given scenarios.
    ///
    /// # Arguments
    ///
    /// * scenarios_to_execute - the scenarios which are going to be executed.
    ///
    /// # Returns
    ///
    /// A vector containing the intersection of processes required to run all the scenarios in the
    /// observation.
    fn collect_processes(
        &self,
        scenarios_to_execute: &[ScenarioToExecute],
    ) -> anyhow::Result<Vec<&ProcessToExecute>> {
        let mut proc_set = std::collections::hash_set::HashSet::new();
        for scenario_to_exec in scenarios_to_execute.iter() {
            proc_set.extend(scenario_to_exec.scenario.processes.iter());
        }

        let mut processes = vec![];
        for proc_name in proc_set {
            let proc = self
                .find_process(proc_name)
                .context(format!("Unable to find process with name: {proc_name}"))?;
            processes.push(proc);
        }

        Ok(processes)
    }

    fn collect_scenarios_to_execute(&self, name: &str) -> anyhow::Result<Vec<ScenarioToExecute>> {
        let mut scenarios = vec![];

        let obs = self.find_observation(name);
        if let Some(obs) = obs {
            // if there is an observation with the given name then get all the scenarios associated
            // with that observation.
            for scenario_name in obs.scenarios.iter() {
                let scenario = self.find_scenario(scenario_name).context(format!(
                    "Unable to find scenario with name: {scenario_name}"
                ))?;
                scenarios.push(scenario);
            }
        } else {
            // if there isn't an observation with the given name then try to find a single scenario
            // with the name instead.
            let scenario = self.find_scenario(name).context(format!(
                "Unable to find observation or scenario with name: {}",
                name
            ))?;
            scenarios.push(scenario);
        }

        let mut scenarios_to_execute = vec![];
        for scenario in scenarios {
            scenarios_to_execute.append(&mut scenario.build_scenarios_to_execute());
        }

        Ok(scenarios_to_execute)
    }

    pub fn create_execution_plan(&self, name: &str) -> anyhow::Result<ExecutionPlan> {
        let scenarios_to_execute = self.collect_scenarios_to_execute(name)?;
        let processes_to_execute = self.collect_processes(&scenarios_to_execute)?;

        Ok(ExecutionPlan {
            processes_to_execute,
            scenarios_to_execute,
            external_processes_to_observe: vec![],
        })
    }

    pub fn create_execution_plan_external_only(&self, name: &str) -> anyhow::Result<ExecutionPlan> {
        let scenarios_to_execute = self.collect_scenarios_to_execute(name)?;

        Ok(ExecutionPlan {
            processes_to_execute: vec![],
            scenarios_to_execute,
            external_processes_to_observe: vec![],
        })
    }
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy, Serialize)]
#[serde(tag = "to", rename_all = "lowercase")]
pub enum Redirect {
    Null,
    Parent,
    File,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Scenario {
    pub name: String,
    pub desc: String,
    pub command: String,
    pub iterations: u32,
    pub processes: Vec<String>,
}
impl Scenario {
    fn build_scenarios_to_execute(&self) -> Vec<ScenarioToExecute> {
        let mut scenarios_to_execute = vec![];
        for i in 0..self.iterations {
            let scenario_to_exec = ScenarioToExecute::new(self, i);
            scenarios_to_execute.push(scenario_to_exec);
        }
        scenarios_to_execute
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ProcessType {
    BareMetal,
    Docker { containers: Vec<String> },
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct ProcessToExecute {
    pub name: String,
    pub up: String,
    pub down: Option<String>,
    pub redirect: Option<Redirect>,
    pub process: ProcessType,
}

#[derive(Debug, Clone)]
pub enum ProcessToObserve {
    Pid(Option<String>, u32),
    ContainerName(String),
}

#[derive(Debug)]
pub struct ScenarioToExecute<'a> {
    pub scenario: &'a Scenario,
    pub iteration: u32,
}
impl<'a> ScenarioToExecute<'a> {
    fn new(scenario: &'a Scenario, iteration: u32) -> Self {
        Self {
            scenario,
            iteration,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Observation {
    pub name: String,
    pub scenarios: Vec<String>,
}

#[derive(Debug)]
pub struct ExecutionPlan<'a> {
    pub processes_to_execute: Vec<&'a ProcessToExecute>,
    pub scenarios_to_execute: Vec<ScenarioToExecute<'a>>,
    pub external_processes_to_observe: Vec<ProcessToObserve>,
}
impl<'a> ExecutionPlan<'a> {
    pub fn scenario_names(&self) -> Vec<&str> {
        self.scenarios_to_execute
            .iter()
            .map(|x| x.scenario.name.as_str())
            .collect()
    }

    /// Adds a process that has not been started by Cardamon to this execution plan for observation.
    ///
    /// # Arguments
    /// * process_to_observe - A process which has been started externally to Cardamon.
    pub fn observe_external_process(&mut self, process_to_observe: ProcessToObserve) {
        self.external_processes_to_observe.push(process_to_observe);
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::*;
    use std::path::Path;

    #[test]
    fn can_load_config_file() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.success.toml"))?;
        assert_eq!(cfg.debug_level, Some("info".to_string()));
        Ok(())
    }

    #[test]
    fn can_find_observation_by_name() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.success.toml"))?;
        let observation = cfg.find_observation("checkout");
        assert!(observation.is_some());

        let observation = cfg.find_observation("nope");
        assert!(observation.is_none());

        Ok(())
    }

    #[test]
    fn can_find_scenario_by_name() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.multiple_scenarios.toml"))?;
        let scenario = cfg.find_scenario("user_signup");
        assert!(scenario.is_some());

        let scenario = cfg.find_scenario("nope");
        assert!(scenario.is_none());

        Ok(())
    }

    #[test]
    fn can_find_process_by_name() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.success.toml"))?;
        let process = cfg.find_process("server");
        assert!(process.is_some());

        let process = cfg.find_process("nope");
        assert!(process.is_none());

        Ok(())
    }

    #[test]
    fn collecting_processes_works() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.multiple_scenarios.toml"))?;
        let scenario1 = cfg
            .find_scenario("user_signup")
            .unwrap()
            .build_scenarios_to_execute();
        let scenario2 = cfg
            .find_scenario("basket_10")
            .unwrap()
            .build_scenarios_to_execute();

        let scenarios_to_execute = vec![scenario1, scenario2]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        let process_names = cfg
            .collect_processes(&scenarios_to_execute)?
            .into_iter()
            .map(|proc| match proc.process {
                ProcessType::BareMetal => proc.name.as_str(),
                ProcessType::Docker { containers: _ } => proc.name.as_str(),
            })
            .sorted()
            .collect::<Vec<_>>();

        assert_eq!(process_names, ["db", "mailgun", "server"]);

        Ok(())
    }

    #[test]
    fn multiple_iterations_should_create_more_scenarios_to_execute() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.multiple_iterations.toml"))?;
        let scenario = cfg
            .find_scenario("basket_10")
            .expect("scenario 'basket_10' should exist!");
        let scenarios_to_execute = scenario.build_scenarios_to_execute();
        assert_eq!(scenarios_to_execute.len(), 2);
        Ok(())
    }

    #[test]
    fn can_create_exec_plan_for_observation() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.multiple_scenarios.toml"))?;

        let exec_plan = cfg.create_execution_plan("checkout")?;
        let scenario_names: Vec<&str> = exec_plan
            .scenarios_to_execute
            .iter()
            .map(|s| s.scenario.name.as_str())
            .sorted()
            .collect();
        let process_names: Vec<&str> = exec_plan
            .processes_to_execute
            .into_iter()
            .map(|proc| match proc.process {
                ProcessType::Docker { containers: _ } => proc.name.as_str(),
                ProcessType::BareMetal => proc.name.as_str(),
            })
            .sorted()
            .collect();

        assert_eq!(scenario_names, ["basket_10", "user_signup"]);
        assert_eq!(process_names, ["db", "mailgun", "server"]);

        Ok(())
    }

    #[test]
    fn can_create_exec_plan_for_scenario() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.multiple_scenarios.toml"))?;

        let exec_plan = cfg.create_execution_plan("basket_10")?;
        let scenario_names: Vec<&str> = exec_plan
            .scenarios_to_execute
            .iter()
            .map(|s| s.scenario.name.as_str())
            .sorted()
            .collect();
        let process_names: Vec<&str> = exec_plan
            .processes_to_execute
            .into_iter()
            .map(|proc| proc.name.as_str())
            .sorted()
            .collect();

        assert_eq!(scenario_names, ["basket_10"]);
        assert_eq!(process_names, ["db", "server"]);

        Ok(())
    }
    #[test]
    fn test_cpu_name_transformations() {
        let test_cases = vec![
            ("AMD Ryzen 7 5800X 8-Core Processor", true, "Ryzen 7 5800X"),
            (
                "Intel(R) Core(TM) i7-7700U CPU @ 2.80GHz",
                true,
                "Core i7-7700U",
            ),
            (
                "Intel(R) Core(TM) i7-13700K CPU @ 2.80GHz",
                true,
                "Core i7-13700K",
            ),
            (
                "Intel(R) Xeon(R) CPU E5-2670 v3 @ 2.30GHz",
                true,
                "Xeon E5-2670",
            ),
            ("AMD EPYC 7742 64-Core Processor", true, "EPYC 7742"),
            ("Unknown CPU Model XYZ", false, "Unknown CPU Model XYZ"),
            ("", false, ""),
            (
                "Intel(R) Core(TM) i5-9400 CPU @ 2.90GHz",
                true,
                "Core i5-9400",
            ),
            ("AMD Ryzen 9 5950X 16-Core Processor", true, "Ryzen 9 5950X"),
            (
                "Intel(R) Xeon(R) Platinum 8272 CPU @ 2.60Ghz",
                true,
                "Xeon Platinum 8272",
            ),
            (
                "Intel(R) Xeon(R) w7-3445 Processor @ 3.70Ghz",
                true,
                "Xeon w7-3445",
            ),
            ("Intel(R) Xeon(R) w7-3445 Processor", true, "Xeon w7-3445"),
        ];

        for (input, should_transform, expected) in test_cases {
            let (transformed, result) = transform_cpu_name(input);
            assert_eq!(
                transformed, should_transform,
                "Transformation flag mismatch for input: {} , expected- {}, got - {result}",
                input, expected
            );
            assert_eq!(result, expected, "Unexpected result for input: {}", input);
        }
    }
    // #[test]
    // fn can_create_scenarios_to_run_for_obs() -> anyhow::Result<()> {
    //     let cfg = Config::from_path(Path::new("./fixtures/cardamon.success.toml"))?;
    //     let obs = cfg.get_observation("checkout")?;
    //     let scenarios_to_run = cfg.scenarios_to_run(obs)?;
    //     assert_eq!(scenarios_to_run.len(), 1);
    //     Ok(())
    // }

    // #[test]
    // fn can_run_an_observation() -> anyhow::Result<()> {
    //     let cfg = Config::from_path(Path::new("./fixtures/cardamon.success.toml"))?;
    //     let run = cfg.run("checkout")?;
    //
    //     // should have 1 scenario to run
    //     let scenarios_to_run = run.scenarios_to_run;
    //     let first = scenarios_to_run
    //         .first()
    //         .context("Should have 1 scenario to run")?;
    //     assert_eq!(scenarios_to_run.len(), 1);
    //     assert_eq!(first.command, "node ./scenarios/basket_10.js");
    //
    //     // should have 2 processes (1 docker with a container name and 1 bare metal with a PID)
    //     match &run.processes_to_observe[..] {
    //         [ProcessToObserve::ContainerName(name), ProcessToObserve::BareMetalId(pid), ..] => {
    //             assert_eq!(name, "postgres");
    //             assert!(*pid > 0);
    //         }
    //         _ => panic!(),
    //     }
    //     Ok(())
    // }

    // #[test]
    // fn cannot_run_misconfigured_observation() -> anyhow::Result<()> {
    //     let cfg = Config::from_path(Path::new("./fixtures/cardamon.missing_process.toml"))?;
    //     let run = cfg.run("checkout");
    //     assert!(run.is_err());
    //
    //     let cfg = Config::from_path(Path::new("./fixtures/cardamon.missing_scenario.toml"))?;
    //     let run = cfg.run("checkout");
    //     assert!(run.is_err());
    //
    //     Ok(())
    // }
}
fn transform_cpu_name(input: &str) -> (bool, String) {
    if input.is_empty() {
        return (false, String::new());
    }

    let amd_ryzen_regex = Regex::new(r"AMD (Ryzen \d+ \d+[A-Z]+)").unwrap();
    let amd_epyc_regex = Regex::new(r"AMD (EPYC \d+)").unwrap();
    let intel_core_regex = Regex::new(r"Intel\(R\) Core\(TM\) (i\d+-\d+[A-Z]*)").unwrap();
    let intel_xeon_regex = Regex::new(r"Intel(?:®|\(R\)) Xeon(?:®|\(R\)) (?:CPU )?(?:Processor )?((?:Platinum |Gold |Silver |[A-Za-z])\d+(?:-\d+)?)").unwrap();

    if let Some(captures) = amd_ryzen_regex.captures(input) {
        (true, captures[1].to_string())
    } else if let Some(captures) = amd_epyc_regex.captures(input) {
        (true, captures[1].to_string())
    } else if let Some(captures) = intel_core_regex.captures(input) {
        (true, format!("Core {}", &captures[1]))
    } else if let Some(captures) = intel_xeon_regex.captures(input) {
        (true, format!("Xeon {}", &captures[1]))
    } else {
        (false, input.to_string())
    }
}
pub async fn calculate_tdp() -> anyhow::Result<u32> {
    let s = System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::everything()));
    let cpu_name = {
        if s.cpus().len() != 0 {
            Some(s.cpus()[0].brand().to_string())
        } else {
            None
        }
    };
    let cpu_name = match cpu_name {
        Some(name) => name,
        None => {
            println!("Could not find CPU automatically, please input CPU name:");
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .context("Failed to read user input")?;
            input
        }
    };
    // CPU names come out as "Intel Core I-7600U .....
    // and AMD Ryzen 7 5800X 8-Core Processor
    // We want Core i-7700U , and Ryzen 7 5800X
    // We need to strip this for the api
    let (regex_worked, cpu_name) = transform_cpu_name(&cpu_name);
    if !regex_worked {
        println!("Regex CPU-Parsing did not work, possible manual intervention required");
    }
    println!("Detected CPU: {}", cpu_name);

    let encoded_cpu_name = form_urlencoded::byte_serialize(cpu_name.as_bytes()).collect::<String>();
    let url = format!(
        "https://www.techpowerup.com/cpu-specs/?ajaxsrch={}",
        encoded_cpu_name
    );

    let response = reqwest::get(&url).await?.text().await?;
    let document = Html::parse_document(&response);

    let row_selector = Selector::parse("table.processors tr")
        .map_err(|e| anyhow::anyhow!("Failed to parse row selector: {}", e))?;
    let rows: Vec<_> = document.select(&row_selector).skip(1).collect();

    if rows.is_empty() {
        println!("No CPU information found for CPU {}", cpu_name);
        return ask_for_manual_tdp();
    }

    let tdp = if rows.len() == 1 {
        let tdp = extract_tdp(&rows[0])?;
        let cpu_name = extract_cpu_name(&rows[0])?;
        println!("Found CPU: {}", cpu_name);
        println!("TDP: {} W", tdp);
        println!("Is this correct? (y/n)");

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .context("Failed to read user input")?;

        if input.trim().to_lowercase() == "y" {
            tdp
        } else {
            ask_for_manual_tdp()?
        }
    } else {
        println!("Multiple CPUs found. Please choose one:");
        for (i, row) in rows.iter().enumerate() {
            println!("{}. {}", i + 1, extract_cpu_name(row)?);
        }

        let choice = loop {
            println!("Enter the number of your CPU or 'm' for manual TDP entry:");
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .context("Failed to read user input")?;

            if input.trim().to_lowercase() == "m" {
                return Ok(ask_for_manual_tdp()?);
            }

            if let Ok(num) = input.trim().parse::<usize>() {
                if num > 0 && num <= rows.len() {
                    break num - 1;
                }
            }
            println!("Invalid input. Please try again.");
        };

        extract_tdp(&rows[choice])?
    };

    Ok(tdp)
}

fn extract_cpu_name(row: &scraper::element_ref::ElementRef) -> anyhow::Result<String> {
    let name_selector = Selector::parse("td:first-child")
        .map_err(|e| anyhow::anyhow!("Failed to parse name selector: {}", e))?;
    Ok(row
        .select(&name_selector)
        .next()
        .context("CPU name not found")?
        .text()
        .collect::<String>())
}

fn extract_tdp(row: &scraper::element_ref::ElementRef) -> anyhow::Result<u32> {
    let tdp_selector = Selector::parse("td:nth-child(8)")
        .map_err(|e| anyhow::anyhow!("Failed to parse TDP selector: {}", e))?;
    let tdp_text = row
        .select(&tdp_selector)
        .next()
        .context("TDP not found")?
        .text()
        .collect::<String>();
    tdp_text
        .trim()
        .replace(" W", "")
        .parse::<u32>()
        .context("Failed to parse TDP")
}

fn ask_for_manual_tdp() -> anyhow::Result<u32> {
    loop {
        println!("Please enter the TDP manually (in watts):");
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .context("Failed to read user input")?;

        match input.trim().parse::<u32>() {
            Ok(tdp) => return Ok(tdp),
            Err(_) => println!("Invalid input. Please enter a valid number."),
        }
    }
}
