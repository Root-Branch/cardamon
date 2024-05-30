pub mod run;

use crate::app_runner;
use anyhow::Context;
use serde::Deserialize;
use std::{fs, io::Read};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub debug_level: String,
    pub metrics_server_url: String,
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

    fn get_observation(&self, observation_name: &str) -> anyhow::Result<&Observation> {
        self.observations
            .iter()
            .find(|obs| obs.name == observation_name)
            .context("Observation with name {name} does not exist.")
    }

    /// Returns a vector of ScenarioToRun for the given observation
    fn scenarios_to_run(
        &self,
        observation: &Observation,
    ) -> anyhow::Result<Vec<run::ScenarioToRun>> {
        let missing_scenarios = observation
            .scenarios
            .iter()
            .filter(|scenario_name| {
                !self
                    .scenarios
                    .iter()
                    .any(|scenario| &&scenario.name == scenario_name)
            })
            .collect::<Vec<_>>();
        if !missing_scenarios.is_empty() {
            let mut err_str = String::new();
            for missing_scenario in missing_scenarios {
                err_str.push_str(missing_scenario);
            }
            return Err(anyhow::anyhow!("scenarios are missing: [{err_str}]"));
        }

        // find all the scenarios listed in the observation
        let obs_scenarios = self
            .scenarios
            .iter()
            .filter(|s| observation.scenarios.contains(&s.name))
            .collect::<Vec<_>>();

        // create a vector of ScenarioToRun for each iteration
        let mut scenarios_to_run = vec![];
        for scenario in obs_scenarios {
            for iteration in 0..observation.iterations {
                let scenario_to_run = run::ScenarioToRun {
                    name: scenario.name.clone(),
                    command: scenario.command.clone(),
                    iteration,
                };
                scenarios_to_run.push(scenario_to_run);
            }
        }

        Ok(scenarios_to_run)
    }

    pub fn run(&self, observation_name: &str) -> anyhow::Result<run::Run> {
        let observation = self.get_observation(observation_name)?;

        // create a vector of scenarios to run for the given observation
        let scenarios_to_run = self.scenarios_to_run(observation)?;

        // launch the application processes returning a vector of ProcessToObserve
        let mut processes_to_observe = vec![];
        for proc_name in observation.processes.iter() {
            let proc = self
                .processes
                .iter()
                .find(|proc| match proc {
                    Process::BareMetal { name, command: _ } => name == proc_name,
                    Process::Docker {
                        name,
                        containers: _,
                        command: _,
                    } => name == proc_name,
                })
                .context("Observation references process that doesn't exist")?;

            let mut procs_to_obs = app_runner::run_process(proc)?;
            processes_to_observe.append(&mut procs_to_obs);
        }

        // return a new Run
        Ok(run::Run {
            processes_to_observe,
            scenarios_to_run,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub desc: String,
    pub command: String,
}

#[derive(Debug, Deserialize)]
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
    pub processes: Vec<String>,
    pub scenarios: Vec<String>,
}

#[cfg(test)]
mod tests {
    use tests::run::ProcessToObserve;

    use super::*;
    use std::path::Path;

    #[test]
    fn can_load_config_file() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.success.toml"))?;
        assert_eq!(cfg.debug_level, "info");
        Ok(())
    }

    #[test]
    fn can_find_observation_by_name() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.success.toml"))?;
        let observation = cfg.get_observation("checkout");
        assert!(observation.is_ok());
        Ok(())
    }

    #[test]
    fn can_create_scenarios_to_run_for_obs() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.success.toml"))?;
        let obs = cfg.get_observation("checkout")?;
        let scenarios_to_run = cfg.scenarios_to_run(obs)?;
        assert_eq!(scenarios_to_run.len(), 1);
        Ok(())
    }

    #[test]
    fn can_run_an_observation() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.success.toml"))?;
        let run = cfg.run("checkout")?;

        // should have 1 scenario to run
        let scenarios_to_run = run.scenarios_to_run;
        let first = scenarios_to_run
            .first()
            .context("Should have 1 scenario to run")?;
        assert_eq!(scenarios_to_run.len(), 1);
        assert_eq!(first.command, "node ./scenarios/basket_10.js");

        // should have 2 processes (1 docker with a container name and 1 bare metal with a PID)
        match &run.processes_to_observe[..] {
            [ProcessToObserve::ContainerName(name), ProcessToObserve::BareMetalId(pid), ..] => {
                assert_eq!(name, "postgres");
                assert!(*pid > 0);
            }
            _ => panic!(),
        }
        Ok(())
    }

    #[test]
    fn multiple_iterations_should_create_more_scenarios_to_run() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.multiple_iterations.toml"))?;
        let obs = cfg.get_observation("checkout")?;
        let scenarios_to_run = cfg.scenarios_to_run(obs)?;
        assert_eq!(scenarios_to_run.len(), 2);
        Ok(())
    }

    #[test]
    fn cannot_run_misconfigured_observation() -> anyhow::Result<()> {
        let cfg = Config::from_path(Path::new("./fixtures/cardamon.missing_process.toml"))?;
        let run = cfg.run("checkout");
        assert!(run.is_err());

        let cfg = Config::from_path(Path::new("./fixtures/cardamon.missing_scenario.toml"))?;
        let run = cfg.run("checkout");
        assert!(run.is_err());

        Ok(())
    }
}
