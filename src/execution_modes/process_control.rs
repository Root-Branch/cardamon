use crate::{
    config::{Process, ProcessType, Redirect},
    execution_modes::execution_plan::ProcessToObserve,
};
use anyhow::{anyhow, Context};
use colored::*;
use std::{fs::OpenOptions, process::Command};
use subprocess::{Exec, NullFile, Redirection};
use tracing::debug;

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
fn run_command_detached(command: &str, redirect: Option<Redirect>) -> anyhow::Result<u32> {
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
                    let out_file = OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open("./.stdout")?;
                    let err_file = OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open("./.stderr")?;
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
pub fn run_process(proc_to_exec: &Process) -> anyhow::Result<ProcessToObserve> {
    match &proc_to_exec.process_type {
        ProcessType::Docker { containers } => {
            debug!(
                "Running command {} in detached mode ( Docker ) ",
                proc_to_exec.up
            );
            // run the command
            run_command_detached(&proc_to_exec.up, proc_to_exec.redirect)?;

            // return the containers as vector of ProcessToObserve
            Ok(ProcessToObserve::ManagedContainers {
                process_name: proc_to_exec.name.clone(),
                container_names: containers.clone(),
                down: proc_to_exec.down.clone(),
            })
        }

        ProcessType::BareMetal => {
            debug!(
                "Running command {} in detached mode ( Bare metal ) ",
                proc_to_exec.up
            );
            // run the command
            let pid = run_command_detached(&proc_to_exec.up, proc_to_exec.redirect)?;

            // return the pid as a ProcessToObserve
            Ok(ProcessToObserve::ManagedPid {
                process_name: proc_to_exec.name.clone(),
                pid,
                down: proc_to_exec
                    .down
                    .clone()
                    .map(|down| down.replace("{pid}", &pid.to_string())),
            })
        }
    }
}

pub fn shutdown_process(running_proc: &ProcessToObserve) -> anyhow::Result<()> {
    match running_proc {
        ProcessToObserve::ManagedPid {
            pid: _,
            process_name,
            down: Some(down),
        } => {
            print!("> stopping process {}", process_name.green());

            // let res = run_command_detached(&down, None);

            let words = shlex::split(&down).expect("Command string is not POSIX compliant.");
            let res = match &words[..] {
                [command, args @ ..] => Command::new(command)
                    .args(args)
                    .output()
                    .map_err(anyhow::Error::from),
                _ => Err(anyhow::anyhow!("Whoops no command!")),
            };

            if res.is_err() {
                let err = res.unwrap_err();
                tracing::warn!(
                    "Failed to shutdown process with name {}\n{}",
                    process_name,
                    err
                );
                println!();
                Err(err)
            } else {
                println!("\t{}", "✓".green());
                println!("\t{}", format!("- {}", down).bright_black());
                Ok(())
            }
        }

        ProcessToObserve::ManagedContainers {
            process_name,
            container_names: _,
            down: Some(down),
        } => {
            print!("> stopping process {}", process_name.green());

            let res = run_command_detached(&down, None);
            if res.is_err() {
                let err = res.unwrap_err();
                tracing::warn!(
                    "Failed to shutdown process with name {}\n{}",
                    process_name,
                    err
                );
                println!();
                Err(err)
            } else {
                println!("\t{}", "✓".green());
                println!("\t{}", format!("- {}", down).bright_black());
                Ok(())
            }
        }

        _ => Ok(()), // do nothing!
    }
}

pub fn shutdown_processes(running_processes: &Vec<ProcessToObserve>) -> anyhow::Result<()> {
    // for each process in the execution plan that has a "down" command, attempt to run that
    // command.
    for proc in running_processes {
        shutdown_process(proc)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_family = "windows")]
    mod windows {
        use super::*;

        #[test]
        fn can_run_a_bare_metal_process() -> anyhow::Result<()> {
            let proc = Process {
                name: "sleep".to_string(),
                up: "powershell sleep 15".to_string(),
                down: None,
                redirect: None,
                process_type: ProcessType::BareMetal,
            };
            let proc_to_observe = run_process(&proc)?;

            match proc_to_observe {
                ProcessToObserve::ManagedPid {
                    process_name: _,
                    pid,
                    down: _,
                } => {
                    let mut system = System::new();
                    system.refresh_all();
                    let proc = system.process(Pid::from_u32(pid));
                    assert!(proc.is_some());
                }

                _ => panic!("expected to find a process id"),
            }

            Ok(())
        }

        #[tokio::test]
        async fn log_scenario_should_return_metrics_log_without_errors() -> anyhow::Result<()> {
            let proc = Process {
                name: "sleep".to_string(),
                up: "powershell sleep 20".to_string(),
                down: None,
                redirect: None,
                process_type: ProcessType::BareMetal,
            };
            let proc_to_observe = run_process(&proc)?;
            let stop_handle = metrics_logger::start_logging(&[&proc_to_observe])?;

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
        use crate::{config::Redirect, metrics_logger};
        use std::{ops::Deref, time::Duration};
        use sysinfo::{Pid, System};

        #[test]
        fn can_run_a_bare_metal_process() -> anyhow::Result<()> {
            let proc = Process {
                name: "sleep".to_string(),
                up: "sleep 15".to_string(),
                down: None,
                redirect: Some(Redirect::Null),
                process_type: ProcessType::BareMetal,
            };
            let proc_to_observe = run_process(&proc)?;

            match proc_to_observe {
                ProcessToObserve::ManagedPid {
                    process_name,
                    pid,
                    down: _,
                } => {
                    let mut system = System::new();
                    system.refresh_all();
                    let proc = system.process(Pid::from_u32(pid));
                    let proc_name = proc.unwrap().name().to_os_string();
                    let proc_name = proc_name.to_string_lossy();
                    let proc_name = proc_name.deref().to_string();
                    assert!(proc.is_some());
                    assert!(proc_name == process_name);
                }

                e => panic!("expected to find a process id {:?}", e),
            }

            Ok(())
        }

        #[tokio::test]
        async fn log_scenario_should_return_metrics_log_without_errors() -> anyhow::Result<()> {
            let proc = Process {
                name: "sleep".to_string(),
                up: "sleep 20".to_string(),
                down: None,
                redirect: Some(Redirect::Null),
                process_type: ProcessType::BareMetal,
            };
            let procs_to_observe = run_process(&proc)?;
            let stop_handle = metrics_logger::start_logging(vec![procs_to_observe])?;

            tokio::time::sleep(Duration::from_secs(10)).await;

            let metrics_log = stop_handle.stop().await?;

            assert!(!metrics_log.has_errors());
            assert!(!metrics_log.get_metrics().is_empty());

            Ok(())
        }
    }
}
