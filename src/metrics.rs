use itertools::Itertools;
use tokio::process::Command;

use std::sync::{Arc, Mutex};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::{metrics_server::dao_schema::scenario, settings::Scenario};

use self::types::CPUStatus;

pub mod bare;
pub mod common;
pub mod docker;
pub mod start;
pub mod types;

pub enum BareMetalProcess {
    ProcId(u32),
    ProcName(String),
}

pub enum DockerProcess {
    ContainerId(String),
    ContainerName(String),
}

pub enum Process {
    BareMetal(BareMetalProcess),
    Docker(DockerProcess),
}

pub async fn log_bare_metal(
    processes: Vec<BareMetalProcess>,
    metrics_log: Arc<Mutex<Vec<CPUStatus>>>,
    token: CancellationToken,
) -> anyhow::Result<()> {
    let mut buffer: Vec<CPUStatus> = vec![];
    let mut i = 0;
    loop {
        tokio::select! {
            _ = async {
                // generate random number (this will be replaced by call to sysinfo)
                for proc in processes.iter() {
                    match proc {
                        BareMetalProcess::ProcId(pid) => match bare::get_stats_pid(*pid).await {
                            Ok(stats) => {
                                buffer.push(stats);
                            }
                            Err(error) => {
                                eprintln!("Error retrieving stats for PID {}: {}", pid, error);
                            }
                        },
                        BareMetalProcess::ProcName(name) => {
                            if let Ok(stats) = bare::get_stats_name(name).await {
                                buffer.push(stats); // TODO: replace with actual data
                            }
                        }
                    }
                }

                // if buffer is full then write to shared metrics log
                if i == 9 {
                    let mut metrics_log = metrics_log.lock().expect("");
                    metrics_log.append(&mut buffer);
                    println!("hello from bare");

                    i = 0;
                    buffer.clear();
                } else {
                    i += 1;
                }

                // simulate waiting for more metrics
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            } => {}
            _ = token.cancelled() => {
                break;
            }
        }
    }

    // Push the remaining buffer to the shared logs when cancelled
    let mut metrics_log = metrics_log.lock().expect("");
    metrics_log.append(&mut buffer);
    println!("log_bare_metal cancelled, pushing remaining buffer");
    Ok(())
}
// TODO: Needs to call metrics::docker::get_docker_stats
pub async fn log_docker(
    _processes: Vec<DockerProcess>,
    _metrics_log: Arc<Mutex<Vec<CPUStatus>>>,
) -> anyhow::Result<()> {
    todo!()
    /*
    let mut buffer: Vec<CPUStatus> = vec![];
    let mut i = 0;
    loop {
        // generate random number (this will be replaced by call to sysinfo)
        // TODO: replace 1338 with actual data
        buffer.push(1338);

        // if buffer is full then write to shared metrics log
        if i == 9 {
            let mut metrics_log = metrics_log.lock().expect("");
            metrics_log.append(&mut buffer);
            println!("hello from docker");

            i = 0;
            buffer.clear();
        } else {
            i += 1;
        }

        // simulate waiting for more metrics
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
        */
}
// TODO: add scenario and persistence service to function signature
pub async fn log_scenario(scenario: Scenario, processes: Vec<Process>) -> anyhow::Result<()> {
    let metrics_log = vec![];
    let shared_metrics_log = Arc::new(Mutex::new(metrics_log));

    // split processes into bare metal & docker processes
    let (bare_metal_procs, docker_procs): (Vec<_>, Vec<_>) =
        processes.into_iter().partition_map(|proc| match proc {
            Process::BareMetal(proc) => itertools::Either::Left(proc),
            Process::Docker(proc) => itertools::Either::Right(proc),
        });

    // create a new cancellation token
    let token = CancellationToken::new();

    // start threads to collect metrics
    let mut join_set = JoinSet::new();
    if !bare_metal_procs.is_empty() {
        let token = token.clone();
        let shared_metrics_log = shared_metrics_log.clone();
        join_set.spawn(async move {
            let _ = log_bare_metal(bare_metal_procs, shared_metrics_log, token.clone()).await;
        });
    }

    if !docker_procs.is_empty() {
        let token = token.clone();
        let shared_metrics_log = shared_metrics_log.clone();
        join_set.spawn(async move {
            tokio::select! {
                _ = token.cancelled() => {}
                _ = log_docker(docker_procs, shared_metrics_log) => {}
            }
        });
    }

    // simulate running the scenarios
    // TODO: make this sleep duration configurable?
    //tokio::time::sleep(Duration::from_secs(30)).await;
    let res = run(&scenario).await;
    match res {
        Ok(_) => println!("Was run successfully"),
        Err(e) => println!("Was not successfully {e}"),
    }
    // TODO: useful during development, remember to remove it!

    // cancel loggers
    token.cancel();

    loop {
        if join_set.join_next().await.is_none() {
            break;
        }
    }
    let logs = shared_metrics_log.lock().expect("").clone();
    println!(" LOGS : {:?}", logs);

    Ok(())
}
async fn run(scenario: &Scenario) -> anyhow::Result<()> {
    // Split the scenario_command into a vector
    let command_parts: Vec<&str> = scenario.command.split_whitespace().collect();

    // Get the command and arguments
    let command = command_parts
        .first()
        .ok_or_else(|| anyhow::anyhow!("Empty command"))?;
    let args = &command_parts[1..];

    // run scenario ...
    let output = Command::new(command)
        .args(args)
        .kill_on_drop(true)
        .output()
        .await?;
    if output.status.success() {
        println!("{:?}", output.stdout);
        Ok(())
    } else {
        println!("{:?}", output.stdout);
        let error_message = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow::anyhow!(
            "Scenario execution failed: {}",
            error_message
        ))
    }
}

// TODO: This should do exactly what log_scenario does but it should save the shared_metrics_log
// at regular fixed intervals (either space or time)
pub async fn log_live(_processes: Vec<Process>) -> anyhow::Result<()> {
    todo!("implement this!")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn metrics_test() -> anyhow::Result<()> {
        log_scenario(
            Scenario {
                name: "name".to_string(),
                iteration: 3,
                command: "sleep 5".to_string(),
            },
            vec![
                Process::BareMetal(BareMetalProcess::ProcId(42)),
                //Process::Docker(DockerProcess::ContainerId(String::from("my_container"))),
            ],
        )
        .await?;

        Ok(())
    }
}
