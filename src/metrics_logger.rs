pub mod bare_metal;
pub mod docker;

use crate::{execution_plan::ProcessToObserve, metrics::CpuMetrics};
use sea_orm::*;
use std::time::Duration;
use tokio::{sync::mpsc, task::JoinSet};
use tokio_util::sync::CancellationToken;

pub struct StopHandle {
    token: CancellationToken,
    pub join_set: JoinSet<()>,
}
impl StopHandle {
    fn new(token: CancellationToken, join_set: JoinSet<()>) -> Self {
        Self { token, join_set }
    }

    pub async fn stop(mut self) {
        // cancel loggers
        self.token.cancel();
        loop {
            if self.join_set.join_next().await.is_none() {
                break;
            }
        }
    }
}

async fn keep_saving(
    queue_rx: &mut mpsc::Receiver<CpuMetrics>,
    run_id: &str,
    db: &DatabaseConnection,
) {
    loop {
        if let Some(metrics) = queue_rx.recv().await {
            println!("{:?}", metrics);
            let _ = metrics.into_active_model(run_id).save(db).await;
        }
        let _ = tokio::time::sleep(Duration::from_secs(2));
    }
}

/// Logs a single scenario run
///
/// # Arguments
///
/// * `processes` - The processes you wish to observe during the scenario run
///
/// # Returns
///
/// A `Result` containing the metrics log for the given scenario or an `Error` if either
/// the scenario failed to complete successfully or any of the loggers contained errors.
pub fn start_logging(
    processes_to_observe: Vec<ProcessToObserve>,
    run_id: String,
    db: DatabaseConnection,
) -> anyhow::Result<StopHandle> {
    // split processes into bare metal & docker processes
    let mut a: Vec<ProcessToObserve> = vec![];
    let mut b: Vec<ProcessToObserve> = vec![];
    for proc in processes_to_observe {
        match proc {
            p @ ProcessToObserve::ExternalPid(_) => a.push(p.clone()),
            p @ ProcessToObserve::ExternalContainers(_) => b.push(p.clone()),

            p @ ProcessToObserve::ManagedPid {
                process_name: _,
                pid: _,
                down: _,
            } => a.push(p.clone()),
            p @ ProcessToObserve::ManagedContainers {
                process_name: _,
                container_names: _,
                down: _,
            } => b.push(p.clone()),
        }
    }

    // create async queue
    let (queue_tx, mut queue_rx) = mpsc::channel::<CpuMetrics>(100);

    // create a new cancellation token
    let cancellation_token = CancellationToken::new();

    // create a new join set for the poducer and consumer threads
    let mut join_set = JoinSet::new();

    // start thread to consume metrics
    let token = cancellation_token.clone();
    join_set.spawn(async move {
        tokio::select! {
            _ = token.cancelled() => {
                while let Some(metrics) = queue_rx.recv().await {
                    let _ = metrics.into_active_model(&run_id).save(&db).await;
                }
            }
            _ = keep_saving(&mut queue_rx, &run_id, &db) => {}
        }
    });

    // start threads to collect metrics
    if !a.is_empty() {
        let token = cancellation_token.clone();
        let queue = queue_tx.clone();

        tracing::debug!("Spawning bare metal thread");
        join_set.spawn(async move {
            tracing::info!("Logging PIDs: {:?}", a);
            tokio::select! {
                _ = token.cancelled() => {}
                _ = bare_metal::keep_logging(a, queue) => {}
            }
        });
    }

    if !b.is_empty() {
        let token = cancellation_token.clone();
        let queue = queue_tx.clone();

        join_set.spawn(async move {
            tracing::info!("Logging containers: {:?}", b);
            tokio::select! {
                _ = token.cancelled() => {}
                _ = docker::keep_logging(b, queue) => {}
            }
        });
    }

    Ok(StopHandle::new(cancellation_token, join_set))
}
