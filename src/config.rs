/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use anyhow::Context;
use serde::Deserialize;
use std::{fs, io::Read};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub debug_level: Option<String>,
    pub metrics_server_url: Option<String>,
    pub processes: Vec<Process>,
    pub scenarios: Vec<Scenario>,
    pub observations: Vec<Observation>,
}
impl Config {
    pub fn from_path(path: &std::path::Path) -> anyhow::Result<Config> {
        let mut config_str = String::new();
        fs::File::open(path)?.read_to_string(&mut config_str)?;

        toml::from_str::<Config>(&config_str).context("Error parsing config file.")
    }

    fn find_observation(&self, observation_name: &str) -> Option<&Observation> {
        self.observations
            .iter()
            .find(|obs| obs.name == observation_name)
    }

    /// Finds a process in the config with the given name.
    ///
    /// # Arguments
    /// * proc_name - the name of the process to find
    ///
    /// # Returns
    /// Some process if it can be found, None otherwise
    fn find_process(&self, proc_name: &str) -> Option<&Process> {
        self.processes.iter().find(|proc| match proc {
            Process::BareMetal { name, command: _ } => name == proc_name,
            Process::Docker {
                name,
                containers: _,
                command: _,
            } => name == proc_name,
        })
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
    ) -> anyhow::Result<Vec<&Process>> {
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

    pub fn create_execution_plan(&self, observation_name: &str) -> anyhow::Result<ExecutionPlan> {
        let observation = self.find_observation(observation_name).context(format!(
            "Unable to find observation with name: {observation_name}"
        ))?;

        let mut scenarios_to_execute = vec![];
        for scenario_name in observation.scenarios.iter() {
            let scenario = self
                .scenarios
                .iter()
                .find(|scenario| &scenario.name == scenario_name)
                .context("")?;

            for i in 0..observation.iterations {
                let scenario_to_run = ScenarioToExecute::new(scenario, i);
                scenarios_to_execute.push(scenario_to_run);
            }
        }

        let processes = self.collect_processes(&scenarios_to_execute)?;

        // return a new Run
        Ok(ExecutionPlan {
            processes,
            scenarios_to_run: scenarios_to_execute,
        })
    }
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Scenario {
    pub name: String,
    pub desc: String,
    pub command: String,
    pub processes: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Process {
    BareMetal {
        name: String,
        command: String,
    },
    Docker {
        name: String,
        containers: Vec<String>,
        command: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct Observation {
    pub name: String,
    pub iterations: u32,
    pub scenarios: Vec<String>,
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

#[derive(Debug)]
pub struct ExecutionPlan<'a> {
    pub processes: Vec<&'a Process>,
    pub scenarios_to_run: Vec<ScenarioToExecute<'a>>,
}
impl<'a> ExecutionPlan<'a> {
    pub fn scenario_names(&self) -> Vec<&str> {
        self.scenarios_to_run
            .iter()
            .map(|x| x.scenario.name.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
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
    fn can_find_process_by_name() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.success.toml"))?;
        let process = cfg.find_process("server");
        assert!(process.is_some());

        let process = cfg.find_process("nope");
        assert!(process.is_none());

        Ok(())
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
    // fn multiple_iterations_should_create_more_scenarios_to_run() -> anyhow::Result<()> {
    //     let cfg = Config::from_path(Path::new("./fixtures/cardamon.multiple_iterations.toml"))?;
    //     let obs = cfg.get_observation("checkout")?;
    //     let scenarios_to_run = cfg.scenarios_to_run(obs)?;
    //     assert_eq!(scenarios_to_run.len(), 2);
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
