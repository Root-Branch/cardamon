pub mod config;
pub mod data_access;
pub mod dataset;
pub mod metrics;
pub mod metrics_logger;

use anyhow::{anyhow, Context};
use config::{ExecutionPlan, ProcessToObserve, ProcessType, Redirect, ScenarioToExecute};
use data_access::{scenario_iteration::ScenarioIteration, DataAccessService};
use dataset::ObservationDataset;
use std::{fs::File, path::Path, time};
use subprocess::{Exec, NullFile, Redirection};
use tracing::info;

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
fn run_command_detached(command: &str, redirect: &Option<Redirect>) -> anyhow::Result<u32> {
    let redirect = redirect.unwrap_or(Redirect::File);

    // break command string into POSIX words
    let words = shlex::split(command).expect("Command string is not POSIX compliant.");

    // split command string into command and args
    match &words[..] {
        [command, args @ ..] => {
            let exec = Exec::cmd(command).args(args);
            // for arg in args {
            //     exec = exec.arg(arg);
            // }
            //

            let exec = match redirect {
                Redirect::Null => exec.stdout(NullFile).stderr(NullFile),
                Redirect::Parent => exec,
                Redirect::File => {
                    let out_file = File::create(Path::new("./.stdout"))?;
                    let err_file = File::create(Path::new("./.stderr"))?;

                    exec.stdout(Redirection::File(out_file))
                        .stderr(Redirection::File(err_file))
                }
            };

            exec.detached()
                .popen()
                .context(format!(
                    "Failed to spawn detached process, command: {}",
                    command
                ))?
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
fn run_process(proc: &config::ProcessToExecute) -> anyhow::Result<Vec<ProcessToObserve>> {
    match &proc.process {
        config::ProcessType::Docker { containers } => {
            // run the command
            run_command_detached(&proc.up, &proc.redirect)?;

            // return the containers as vector of ProcessToObserve
            Ok(containers
                .iter()
                .map(|name| ProcessToObserve::ContainerName(name.clone()))
                .collect())
        }

        config::ProcessType::BareMetal => {
            // run the command
            let pid = run_command_detached(&proc.up, &proc.redirect)?;

            // return the pid as a ProcessToObserve
            Ok(vec![ProcessToObserve::Pid(Some(proc.name.clone()), pid)])
        }
    }
}

async fn run_scenario<'a>(
    run_id: &str,
    scenario_to_execute: &ScenarioToExecute<'a>,
) -> anyhow::Result<ScenarioIteration> {
    let start = time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)?
        .as_millis();

    // Split the scenario_command into a vector
    let command_parts: Vec<&str> = scenario_to_execute
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
    println!(
        "Running scenario {} iteration {}",
        scenario_to_execute.scenario.name,
        scenario_to_execute.iteration + 1
    );
    let output = tokio::process::Command::new(command)
        .args(args)
        .kill_on_drop(true)
        .output()
        .await
        .context(format!("Tokio command failed to run {command}"))?;
    info!("Ran command {}", scenario_to_execute.scenario.command);
    if output.status.success() {
        let stop = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)?
            .as_millis();

        let scenario_iteration = ScenarioIteration::new(
            run_id,
            &scenario_to_execute.scenario.name,
            scenario_to_execute.iteration as i64,
            start as i64,
            stop as i64,
        );
        Ok(scenario_iteration)
    } else {
        let error_message = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow::anyhow!(
            "Scenario execution failed: {}. Command: {}",
            error_message,
            scenario_to_execute.scenario.command
        ))
    }
}

fn shutdown_application(
    exec_plan: &ExecutionPlan,
    running_processes: &[ProcessToObserve],
) -> anyhow::Result<()> {
    // for each process in the execution plan that has a "down" command, attempt to run that
    // command.
    for proc in exec_plan.processes_to_execute.iter() {
        if let Some(down_command) = &proc.down {
            match proc.process {
                ProcessType::BareMetal => {
                    // find the pid associated with this process
                    let pid = running_processes.iter().find_map(|p| match p {
                        ProcessToObserve::Pid(Some(name), pid) if name == &proc.name => Some(*pid),
                        _ => None,
                    });

                    // if pid can't be found then log an error
                    if let Some(pid) = pid {
                        // replace {pid} with the actual PID in the down command
                        let down_command = down_command.replace("{pid}", &pid.to_string());

                        let res = run_command_detached(&down_command, &proc.redirect);
                        if res.is_err() {
                            let err = res.unwrap_err();
                            tracing::warn!(
                                "Failed to shutdown process with name {}\n{}",
                                proc.name,
                                err
                            );
                        }
                    } else {
                        tracing::warn!(
                            "Unable to find PID for bare-metal process with name: {}",
                            proc.name
                        );
                    }
                }
                ProcessType::Docker { containers: _ } => {
                    let res = run_command_detached(down_command, &proc.redirect);
                    if res.is_err() {
                        let err = res.unwrap_err();
                        tracing::warn!(
                            "Failed to shutdown process with name {}\n{}",
                            proc.name,
                            err
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

pub async fn run<'a>(
    exec_plan: ExecutionPlan<'a>,
    data_access_service: &dyn DataAccessService,
) -> anyhow::Result<ObservationDataset> {
    // create a unique cardamon run id
    let run_id = nanoid::nanoid!(5);

    let mut processes_to_observe = exec_plan.external_processes_to_observe.to_vec(); // external procs to observe are cloned here.

    // run the application if there is anything to run
    if !exec_plan.processes_to_execute.is_empty() {
        for proc in exec_plan.processes_to_execute.iter() {
            let process_to_observe = run_process(proc)?;
            processes_to_observe.extend(process_to_observe);
        }
    }

    // record the cardamon run
    let start_time = time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)?
        .as_millis() as i64;
    let mut run = data_access::run::Run::new(&run_id, start_time);
    data_access_service.run_dao().persist(&run).await?;

    // ---- for each scenario ----
    for scenario_to_execute in exec_plan.scenarios_to_execute.iter() {
        // start the metrics loggers
        let stop_handle = metrics_logger::start_logging(&processes_to_observe)?;

        // run the scenario
        let scenario_iteration = run_scenario(&run_id, scenario_to_execute).await?;

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
            .scenario_iteration_dao()
            .persist(&scenario_iteration)
            .await?;

        for metrics in metrics_log.get_metrics() {
            data_access_service
                .cpu_metrics_dao()
                .persist(&metrics.into_data_access(&run_id))
                .await?;
        }
    }
    // ---- end for ----

    // update run stop time
    let stop_time = time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)?
        .as_millis() as i64;
    run.stop(stop_time);
    data_access_service.run_dao().persist(&run).await?;

    // stop the application
    shutdown_application(&exec_plan, &processes_to_observe)?;

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
    use crate::{
        config::{ProcessToExecute, ProcessType},
        metrics_logger, run_process, ProcessToObserve,
    };
    use std::time::Duration;
    use sysinfo::{Pid, System};

    #[cfg(target_family = "windows")]
    mod windows {
        use super::*;

        #[test]
        fn can_run_a_bare_metal_process() -> anyhow::Result<()> {
            let process = ProcessToExecute {
                name: "sleep".to_string(),
                up: "powershell sleep 15".to_string(),
                down: None,
                redirect: None,
                process: ProcessType::BareMetal,
            };
            let processes_to_observe = run_process(&process)?;

            assert_eq!(processes_to_observe.len(), 1);

            match processes_to_observe.first().expect("process should exist") {
                ProcessToObserve::Pid(_, pid) => {
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
        async fn log_scenario_should_return_metrics_log_without_errors() -> anyhow::Result<()> {
            let process = ProcessToExecute {
                name: "sleep".to_string(),
                up: "powershell sleep 20".to_string(),
                down: None,
                redirect: None,
                process: ProcessType::BareMetal,
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

    #[cfg(target_family = "unix")]
    mod unix {
        use super::*;
        use crate::config::Redirect;

        #[test]
        fn can_run_a_bare_metal_process() -> anyhow::Result<()> {
            let process = ProcessToExecute {
                name: "sleep".to_string(),
                up: "sleep 15".to_string(),
                down: None,
                redirect: Some(Redirect::Null),
                process: ProcessType::BareMetal,
            };
            let processes_to_observe = run_process(&process)?;

            assert_eq!(processes_to_observe.len(), 1);

            match processes_to_observe.first().expect("process should exist") {
                ProcessToObserve::Pid(_, pid) => {
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
        async fn log_scenario_should_return_metrics_log_without_errors() -> anyhow::Result<()> {
            let process = ProcessToExecute {
                name: "sleep".to_string(),
                up: "sleep 20".to_string(),
                down: None,
                redirect: Some(Redirect::Null),
                process: ProcessType::BareMetal,
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
}
