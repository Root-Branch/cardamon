use anyhow::Context;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
};

#[cfg(not(windows))]
static EXAMPLE_CONFIG: &str = include_str!("templates/cardamon.unix.toml");
#[cfg(windows)]
static EXAMPLE_CONFIG: &str = include_str!("templates/cardamon.win.toml");

#[cfg(not(windows))]
static LINE_ENDING: &str = "\n";
#[cfg(windows)]
static LINE_ENDING: &str = "\r\n";

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub metrics_server_url: Option<String>,
    pub computer: Computer,
    pub processes: Vec<ProcessToExecute>,
    pub scenarios: Vec<Scenario>,
    pub observations: Vec<Observation>,
}
impl Config {
    pub fn write_example_to_file(
        cpu_name: &str,
        cpu_avg_power: f64,
        path: &std::path::Path,
    ) -> anyhow::Result<File> {
        // remove the line containing tdp
        let mut lines = EXAMPLE_CONFIG.lines().map(|s| s.to_string()).collect_vec();

        // add a line at the top of the file containing the new tdp
        let mut new_conf_lines = vec![
            "[computer]".to_string(),
            format!("cpu_name = \"{}\"", cpu_name),
            format!("cpu_avg_power = {}", cpu_avg_power),
            "".to_string(),
        ];
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

        // let mut scenarios_to_execute = vec![];
        // for scenario in scenarios {
        //     scenarios_to_execute.append(&mut scenario.build_scenarios_to_execute());
        // }
        let scenarios_to_execute = scenarios
            .iter()
            .map(|sc| ScenarioToExecute::new(sc))
            .collect_vec();

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

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Computer {
    pub cpu_name: String,
    pub cpu_avg_power: f32,
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
    pub iterations: i32,
    pub processes: Vec<String>,
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
}
impl<'a> ScenarioToExecute<'a> {
    fn new(scenario: &'a Scenario) -> Self {
        Self { scenario }
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
        let cfg = Config::try_from_path(Path::new("./fixtures/cardamon.success.toml"))?;
        assert_eq!(
            cfg.metrics_server_url,
            Some("http://cardamon.rootandbranch.io".to_string())
        );
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
        assert!(scenario.is_some());

        let scenario = cfg.find_scenario("nope");
        assert!(scenario.is_none());

        Ok(())
    }

    #[test]
    fn can_find_process_by_name() -> anyhow::Result<()> {
        let cfg = Config::try_from_path(Path::new("./fixtures/cardamon.success.toml"))?;
        let process = cfg.find_process("server");
        assert!(process.is_some());

        let process = cfg.find_process("nope");
        assert!(process.is_none());

        Ok(())
    }

    #[test]
    fn collecting_processes_works() -> anyhow::Result<()> {
        let cfg = Config::try_from_path(Path::new("./fixtures/cardamon.multiple_scenarios.toml"))?;
        let scenario1 = cfg.find_scenario("user_signup").unwrap();
        let scenario2 = cfg.find_scenario("basket_10").unwrap();

        let scenarios_to_execute = vec![scenario1, scenario2]
            .iter()
            .map(|sc| ScenarioToExecute::new(sc))
            .collect_vec();

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
        let cfg = Config::try_from_path(Path::new("./fixtures/cardamon.multiple_iterations.toml"))?;
        let scenario = cfg
            .find_scenario("basket_10")
            .expect("scenario 'basket_10' should exist!");
        let scenarios_to_execute = vec![ScenarioToExecute::new(scenario)];
        assert_eq!(scenarios_to_execute.len(), 2);
        Ok(())
    }

    #[test]
    fn can_create_exec_plan_for_observation() -> anyhow::Result<()> {
        let cfg = Config::try_from_path(Path::new("./fixtures/cardamon.multiple_scenarios.toml"))?;

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
        let cfg = Config::try_from_path(Path::new("./fixtures/cardamon.multiple_scenarios.toml"))?;

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
}
