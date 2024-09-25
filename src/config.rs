use anyhow::Context;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
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

    fn find_observation(&self, obs_name: &str) -> Option<&Observation> {
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
            let scenario = self.find_scenario(&scenario_name)?;
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

    fn find_processes(&self, proc_names: &[&String]) -> anyhow::Result<Vec<&Process>> {
        let mut processes = vec![];
        for proc_name in proc_names {
            let proc = self.find_process(&proc_name)?;
            processes.push(proc);
        }
        Ok(processes)
    }

    pub fn create_execution_plan(
        &self,
        cpu: Cpu,
        obs_name: &str,
        external_only: bool,
    ) -> anyhow::Result<ExecutionPlan> {
        let obs = self.find_observation(obs_name).context(format!(
            "Couldn't find an observation with name {}",
            obs_name
        ))?;

        let mut processes_to_execute = vec![];

        let exec_plan = match &obs {
            Observation::ScenarioRunner { name: _, scenarios } => {
                let scenario_names = scenarios.iter().collect_vec();
                let scenarios = self.find_scenarios(&scenario_names)?;

                // find the intersection of processes between all the scenarios
                if !external_only {
                    let mut proc_set: HashSet<String> = HashSet::new();
                    for scenario_name in scenario_names {
                        let scenario = self.find_scenario(&scenario_name).context(format!(
                            "Unable to find scenario with name {}",
                            scenario_name
                        ))?;
                        for proc_name in &scenario.processes {
                            proc_set.insert(proc_name.clone());
                        }
                    }

                    let proc_names = proc_set.iter().collect_vec();
                    processes_to_execute = self.find_processes(&proc_names)?;
                }
                ExecutionPlan::new(
                    cpu,
                    processes_to_execute,
                    ExecutionMode::Observation(scenarios),
                )
            }

            Observation::LiveMonitor { name: _, processes } => {
                if !external_only {
                    let proc_names = processes.iter().collect_vec();
                    processes_to_execute = self.find_processes(&proc_names)?;
                }
                ExecutionPlan::new(cpu, processes_to_execute, ExecutionMode::Live)
            }
        };

        Ok(exec_plan)
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

// #[derive(Debug, Deserialize, Serialize)]
// pub struct Observation {
//     pub name: String,
//     #[serde(rename = "observe")]
//     pub observation_mode: ObservationMode,
// }

// ******** ******** ********
// **    EXECUTION PLAN    **
// ******** ******** ********
#[derive(Debug, Clone)]
pub enum ProcessToObserve {
    ExternalPid(u32),
    ExternalContainers(Vec<String>),

    /// ManagedPid represents a baremetal processes started by Cardamon
    ManagedPid {
        process_name: String,
        pid: u32,
        down: Option<String>,
    },

    /// ManagedContainers represents a docker processes started by Cardamon
    ManagedContainers {
        process_name: String,
        container_names: Vec<String>,
        down: Option<String>,
    },
}

#[derive(Debug)]
pub enum ExecutionMode<'a> {
    Live,
    Observation(Vec<&'a Scenario>),
}

#[derive(Debug)]
pub struct ExecutionPlan<'a> {
    pub cpu: Cpu,
    pub external_processes_to_observe: Option<Vec<ProcessToObserve>>,
    pub processes_to_execute: Vec<&'a Process>,
    pub execution_mode: ExecutionMode<'a>,
}
impl<'a> ExecutionPlan<'a> {
    pub fn new(
        cpu: Cpu,
        processes_to_execute: Vec<&'a Process>,
        execution_mode: ExecutionMode<'a>,
    ) -> Self {
        ExecutionPlan {
            cpu,
            external_processes_to_observe: None,
            processes_to_execute,
            execution_mode,
        }
    }

    /// Adds a process that has not been started by Cardamon to this execution plan for observation.
    ///
    /// # Arguments
    /// * process_to_observe - A process which has been started externally to Cardamon.
    pub fn observe_external_process(&mut self, process_to_observe: ProcessToObserve) {
        match &mut self.external_processes_to_observe {
            None => self.external_processes_to_observe = Some(vec![process_to_observe]),
            Some(vec) => {
                vec.push(process_to_observe);
                Some(vec);
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::*;
    use std::path::Path;

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

    // #[test]
    // fn collecting_processes_works() -> anyhow::Result<()> {
    //     let cfg = Config::try_from_path(Path::new("./fixtures/cardamon.multiple_scenarios.toml"))?;
    //
    //     let obs_name = "test_app";
    //     let obs = cfg.find_observation(obs_name).context("")?;
    //
    //     let process_names = cfg
    //         .collect_processes(obs.)?
    //         .into_iter()
    //         .map(|proc_to_exec| match proc_to_exec.process.process_type {
    //             ProcessType::BareMetal => proc_to_exec.process.name.as_str(),
    //             ProcessType::Docker { containers: _ } => proc_to_exec.process.name.as_str(),
    //         })
    //         .sorted()
    //         .collect::<Vec<_>>();
    //
    //     assert_eq!(process_names, ["db", "mailgun", "server"]);
    //
    //     Ok(())
    // }

    // #[test]
    // fn multiple_iterations_should_create_more_scenarios_to_execute() -> anyhow::Result<()> {
    //     let cfg = Config::try_from_path(Path::new("./fixtures/cardamon.multiple_iterations.toml"))?;
    //     let scenario = cfg
    //         .find_scenario("basket_10")
    //         .expect("scenario 'basket_10' should exist!");
    //     let scenarios_to_execute = vec![ScenarioToExecute::new(scenario)];
    //     assert_eq!(scenarios_to_execute.len(), 2);
    //     Ok(())
    // }

    #[test]
    fn can_create_exec_plan_for_observation() -> anyhow::Result<()> {
        let cfg = Config::try_from_path(Path::new("./fixtures/cardamon.multiple_scenarios.toml"))?;

        let cpu = Cpu {
            name: "AMD Ryzen 7 6850U".to_string(),
            power: Power::Tdp(11.2),
        };

        let exec_plan = cfg.create_execution_plan(cpu, "checkout", false)?;
        match exec_plan.execution_mode {
            ExecutionMode::Observation(scenarios) => {
                let scenario_names = scenarios
                    .iter()
                    .map(|s| s.name.as_str())
                    .sorted()
                    .collect_vec();

                let process_names: Vec<&str> = exec_plan
                    .processes_to_execute
                    .into_iter()
                    .map(|proc| match proc.process_type {
                        ProcessType::Docker { containers: _ } => proc.name.as_str(),
                        ProcessType::BareMetal => proc.name.as_str(),
                    })
                    .sorted()
                    .collect();

                assert_eq!(scenario_names, ["basket_10", "user_signup"]);
                assert_eq!(process_names, ["db", "mailgun", "server"]);
            }

            _ => panic!("oops! was expecting a ObservationMode::Scenarios"),
        }

        Ok(())
    }

    #[test]
    fn can_create_exec_plan_for_monitor() -> anyhow::Result<()> {
        let cfg = Config::try_from_path(Path::new("./fixtures/cardamon.multiple_scenarios.toml"))?;

        let cpu = Cpu {
            name: "AMD Ryzen 7 6850U".to_string(),
            power: Power::Tdp(11.2),
        };

        let exec_plan = cfg.create_execution_plan(cpu, "live_monitor", false)?;
        match exec_plan.execution_mode {
            ExecutionMode::Live => {
                let process_names: Vec<&str> = exec_plan
                    .processes_to_execute
                    .into_iter()
                    .map(|proc| proc.name.as_str())
                    .sorted()
                    .collect();

                assert_eq!(process_names, ["db", "mailgun", "server"]);
            }

            _ => panic!("oops! was expecting a ObservationMode::Monitor"),
        }
        Ok(())
    }
}
