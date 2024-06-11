pub mod config;
pub mod data_access;
pub mod dataset;
pub mod metrics;
pub mod metrics_logger;

use anyhow::{anyhow, Context};
use config::{ExecutionPlan, ScenarioToRun};
use data_access::{scenario_run::ScenarioRun, DataAccessService};
use dataset::ObservationDataset;
use std::time;
use subprocess::{Exec, NullFile};

pub enum ProcessToObserve {
    ProcId(u32),
    ContainerName(String),
}

/// Runs the given command as a detached processes. This function does not block because the
/// process is managed by the OS and running separately from this thread.
///
/// # Arguments
///
/// * command - The command to run.
///
/// # Returns
///
/// The PID returned by the operating system
fn run_command_detached(command: &str) -> anyhow::Result<u32> {
    // split command string into command and args
    match &command.split(' ').collect::<Vec<_>>()[..] {
        [command, args @ ..] => {
            let mut exec = Exec::cmd(command);
            for arg in args {
                exec = exec.arg(arg);
            }

            exec.detached()
                .stdout(NullFile)
                .popen()
                .context("Failed to spawn detached process")?
                .pid()
                .context("Process should have a PID")
        }
        _ => Err(anyhow!("")),
    }
}

/// Run the given process as a detached process and return a list of all things to observe (in
/// Docker it's possible to have a single docker compose process which starts multiple containers).
///
/// # Arguments
///
/// * proc - The Process to run
///
/// # Returns
///
/// A list of all the processes to observe
fn run_process(proc: &config::Process) -> anyhow::Result<Vec<ProcessToObserve>> {
    match proc {
        config::Process::Docker {
            name: _,
            containers,
            command,
        } => {
            // run the command
            run_command_detached(command)?;

            // return the containers as vector of ProcessToObserve
            Ok(containers
                .iter()
                .map(|name| ProcessToObserve::ContainerName(name.clone()))
                .collect())
        }

        config::Process::BareMetal { name: _, command } => {
            // run the command
            let pid = run_command_detached(command)?;

            // return the pid as a ProcessToObserve
            Ok(vec![ProcessToObserve::ProcId(pid)])
        }
    }
}

async fn run_scenario<'a>(
    cardamon_run_id: &str,
    scenario_to_run: &ScenarioToRun<'a>,
) -> anyhow::Result<ScenarioRun> {
    let start = time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)?
        .as_millis();

    // Split the scenario_command into a vector
    let command_parts: Vec<&str> = scenario_to_run
        .scenario
        .command
        .split_whitespace()
        .collect();

    // Get the command and arguments
    let command = command_parts
        .first()
        .ok_or_else(|| anyhow::anyhow!("Empty command"))?;
    let args = &command_parts[1..];

    // run scenario ...
    let output = tokio::process::Command::new(command)
        .args(args)
        .kill_on_drop(true)
        .output()
        .await?;

    if output.status.success() {
        let stop = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)?
            .as_millis();

        let scenario_run = ScenarioRun::new(
            cardamon_run_id,
            &scenario_to_run.scenario.name,
            scenario_to_run.iteration as i64,
            start as i64,
            stop as i64,
        );
        Ok(scenario_run)
    } else {
        let error_message = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow::anyhow!(
            "Scenario execution failed: {}",
            error_message
        ))
    }
}

pub async fn run<'a>(
    exec_plan: ExecutionPlan<'a>,
    data_access_service: &dyn DataAccessService,
) -> anyhow::Result<ObservationDataset> {
    // create a unique cardamon run id
    let cardamon_run_id = nanoid::nanoid!(5);

    // run the application
    let mut processes_to_observe = vec![];
    for proc in exec_plan.processes.iter() {
        let process_to_observe = run_process(proc)?;
        processes_to_observe.extend(process_to_observe);
    }

    // ---- for each scenario ----
    for scenario_to_run in exec_plan.scenarios_to_run.iter() {
        // start the metrics loggers
        let stop_handle = metrics_logger::start_logging(&processes_to_observe)?;

        // run the scenario
        let scenario_run = run_scenario(&cardamon_run_id, scenario_to_run).await?;

        // stop the metrics loggers
        let metrics_log = stop_handle.stop().await?;

        // if metrics log contains errors then display them to the user and don't save anything
        if metrics_log.has_errors() {
            // log all the errors
            for err in metrics_log.get_errors() {
                tracing::error!("{}", err);
            }
            return Err(anyhow!("Metric log contained errors, please see logs."));
        }

        // write scenario and metrics to db
        data_access_service
            .scenario_run_dao()
            .persist(&scenario_run)
            .await?;

        for metrics in metrics_log.get_metrics() {
            data_access_service
                .cpu_metrics_dao()
                .persist(&metrics.into_data_access(&cardamon_run_id))
                .await?;
        }
    }
    // ---- end for ----

    // stop the application
    // TODO: Implement this!

    // create a summary to return to the user
    let scenario_names = exec_plan.scenario_names();
    let previous_runs = 3;
    let observation_dataset = data_access_service
        .fetch_observation_dataset(scenario_names, previous_runs)
        .await?;

    Ok(observation_dataset)
}

#[cfg(test)]
mod tests {
    use crate::{config::Process, metrics_logger, run_process, ProcessToObserve};
    use std::time::Duration;
    use sysinfo::{Pid, System};

    #[test]
    #[cfg(target_family = "windows")]
    fn can_run_a_bare_metal_process() -> anyhow::Result<()> {
        let process = Process::BareMetal {
            name: "sleep".to_string(),
            command: "powershell sleep 15".to_string(),
        };
        let processes_to_observe = run_process(&process)?;

        assert_eq!(processes_to_observe.len(), 1);

        match processes_to_observe.first().expect("process should exist") {
            ProcessToObserve::ProcId(pid) => {
                let mut system = System::new();
                system.refresh_all();
                let proc = system.process(Pid::from_u32(*pid));
                assert!(proc.is_some());
            }

            _ => panic!("expected to find a process id"),
        }

        Ok(())
    }

    #[tokio::test]
    #[cfg(target_family = "windows")]
    async fn log_scenario_should_return_metrics_log_without_errors() -> anyhow::Result<()> {
        let process = Process::BareMetal {
            name: "sleep".to_string(),
            command: "powershell sleep 20".to_string(),
        };
        let processes_to_observe = run_process(&process)?;
        let stop_handle = metrics_logger::start_logging(&processes_to_observe)?;

        tokio::time::sleep(Duration::from_secs(10)).await;

        let metrics_log = stop_handle.stop().await?;

        assert!(!metrics_log.has_errors());
        assert!(!metrics_log.get_metrics().is_empty());

        Ok(())
    }
}
