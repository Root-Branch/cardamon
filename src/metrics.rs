use itertools::Itertools;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

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
    metrics_log: Arc<Mutex<Vec<i32>>>,
) -> anyhow::Result<()> {
    let mut buffer: Vec<i32> = vec![];
    let mut i = 0;
    loop {
        // generate random number (this will be replaced by call to sysinfo)
        for proc in processes.iter() {
            match proc {
                BareMetalProcess::ProcId(pid) => {
                    if let Ok(_stats) = bare::get_stats_pid(*pid).await {
                        buffer.push(1337); // TODO:: replace with actual data
                    }
                }
                BareMetalProcess::ProcName(name) => {
                    if let Ok(_stats) = bare::get_stats_name(name).await {
                        buffer.push(2337); // TODO: replace with actual data
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
    }
}

// TODO: Needs to call metrics::docker::get_docker_stats
pub async fn log_docker(
    _processes: Vec<DockerProcess>,
    metrics_log: Arc<Mutex<Vec<i32>>>,
) -> anyhow::Result<()> {
    let mut buffer: Vec<i32> = vec![];
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
}

// TODO: add scenario and persistence service to function signature
pub async fn log_scenario(processes: Vec<Process>) -> anyhow::Result<()> {
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
            tokio::select! {
                _ = token.cancelled() => {}
                _ = log_bare_metal(bare_metal_procs, shared_metrics_log) => {}
            }
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
    tokio::time::sleep(Duration::from_secs(30)).await;

    // TODO: useful during development, remember to remove it!
    println!("{:?}", shared_metrics_log.lock().expect(""));

    // cancel loggers
    token.cancel();

    loop {
        if join_set.join_next().await.is_none() {
            break;
        }
    }

    Ok(())
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
        log_scenario(vec![
            Process::BareMetal(BareMetalProcess::ProcId(42)),
            Process::Docker(DockerProcess::ContainerId(String::from("my_container"))),
        ])
        .await?;

        Ok(())
    }
}
