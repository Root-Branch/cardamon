use itertools::Itertools;
use std::future::Future;
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

pub async fn log_bare_metal(processes: Vec<BareMetalProcess>, metrics_log: Mutex<Vec<Metrics>>) -> anyhow::Result<()> {
    tokio::time::sleep(std::time::Duration::from_secs(20));

    let mut vec = vec![];

    loop {
        // call sysinfo
        // batch metrics
        metrics_log.lock().push(.....)
        todo!("save metrics to db");
    }

    todo!()
}

pub async fn log_docker(processes: Vec<DockerProcess>, metrics_log: Mutex<Vec<Metrics>>) -> anyhow::Result<Vec<i32>> {
    todo!()
}

pub async fn log_scenario(processes: Vec<Process>, scenario: Scenario, persistence_service: ) -> anyhow::Result<()> {
    let metrics_log = vec![];
    let shared_metrics_log = Mutex::new(metrics_log);

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
        join_set.spawn(async move {
            tokio::select! {
                _ = token.cancelled() => {}
                _ = log_bare_metal(bare_metal_procs, shared_metrics_log) => {}
            }
        });
    }

    if !docker_procs.is_empty() {
        let token = token.clone();
        join_set.spawn(async move {
            tokio::select! {
                _ = token.cancelled() => {}
                _ = log_docker(docker_procs, shared_metric_log) => {}
            }
        });
    }

    let res = scenario.run().await;

    // persist metrics_log

    // run the command to start the application
    match res {
        Ok(scenario) => {
            // do nothing or add scenario to database
            token.cancel();
            scenario.save();
        }
        Err(error) => {
            // handle error and/or DO NOT add scenario to database
            token.cancel();
        }
    };

    loop {
        join_set.join_next().await;
    }
}

pub async fn log_live(processes: Vec<Process>, persistence_service: ) -> anyhow::Result<()> {

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let cardamon_run_id = "ughry75hdkf0l";

        let processes = vec![Process::BareMetal(BareMetalProcess::ProcId(42))];
        log(processes, asnyc |tx| {
            let start_time = now();
            let mut i = 0;

            // run the scenario
            
            i += 1;
            let stop_time = now();

            // create scenario
            // save scenario (using start and stop time)
        });






















    }
}
