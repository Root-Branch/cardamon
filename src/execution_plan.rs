use super::ExecutionMode;
use crate::config::{Config, Cpu, Observation, Process};
use anyhow::Context;
use itertools::*;
use std::collections::HashSet;

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
            Some(vec) => vec.push(process_to_observe),
        };
    }
}

pub fn create_execution_plan<'a>(
    config: &'a Config,
    cpu: Cpu,
    obs_name: &str,
    external_only: bool,
    daemon: bool,
) -> anyhow::Result<ExecutionPlan<'a>> {
    let obs = config.find_observation(obs_name).context(format!(
        "Couldn't find an observation with name {}",
        obs_name
    ))?;

    let mut processes_to_execute = vec![];

    let exec_plan = match &obs {
        Observation::ScenarioRunner { name: _, scenarios } => {
            let scenario_names = scenarios.iter().collect_vec();
            let scenarios = config.find_scenarios(&scenario_names)?;

            // find the intersection of processes between all the scenarios
            if !external_only {
                let mut proc_set: HashSet<String> = HashSet::new();
                for scenario_name in scenario_names {
                    let scenario = config.find_scenario(scenario_name).context(format!(
                        "Unable to find scenario with name {}",
                        scenario_name
                    ))?;
                    for proc_name in &scenario.processes {
                        proc_set.insert(proc_name.clone());
                    }
                }

                let proc_names = proc_set.iter().collect_vec();
                processes_to_execute = config.find_processes(&proc_names)?;
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
                processes_to_execute = config.find_processes(&proc_names)?;
            }

            let exec_mode = if daemon {
                ExecutionMode::Daemon
            } else {
                ExecutionMode::Live
            };

            ExecutionPlan::new(cpu, processes_to_execute, exec_mode)
        }
    };

    Ok(exec_plan)
}

#[cfg(test)]
mod tests {
    use crate::config::{Power, ProcessType};
    use std::path::Path;

    use super::*;

    #[test]
    fn can_create_exec_plan_for_observation() -> anyhow::Result<()> {
        let cfg = Config::try_from_path(Path::new("./fixtures/cardamon.multiple_scenarios.toml"))?;

        let cpu = Cpu {
            name: "AMD Ryzen 7 6850U".to_string(),
            power: Power::Tdp(11.2),
        };

        let exec_plan = create_execution_plan(&cfg, cpu, "checkout", false, false)?;
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

        let exec_plan = create_execution_plan(&cfg, cpu, "live_monitor", false, false)?;
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
