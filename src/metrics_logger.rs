pub mod bare_metal;
pub mod docker;

use crate::{execution_plan::ProcessToObserve, metrics::MetricsLog};
use std::sync::{Arc, Mutex};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

pub struct StopHandle {
    pub token: CancellationToken,
    pub join_set: JoinSet<()>,
    pub shared_metrics_log: Arc<Mutex<MetricsLog>>,
}
impl StopHandle {
    fn new(
        token: CancellationToken,
        join_set: JoinSet<()>,
        shared_metrics_log: Arc<Mutex<MetricsLog>>,
    ) -> Self {
        Self {
            token,
            join_set,
            shared_metrics_log,
        }
    }

    pub async fn stop(mut self) -> anyhow::Result<MetricsLog> {
        // cancel loggers
        self.token.cancel();
        loop {
            if self.join_set.join_next().await.is_none() {
                break;
            }
        }

        // take ownership of metrics log
        let metrics_log = Arc::try_unwrap(self.shared_metrics_log)
            .expect("Mutex guarding metrics_log shouldn't have multiple owners!")
            .into_inner()
            .expect("Should be able to take ownership of metrics_log");

        // return error if metrics log contains any errors
        if metrics_log.has_errors() {
            return Err(anyhow::anyhow!(
                "Metrics log contains errors, please check trace"
            ));
        }

        Ok(metrics_log)
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
pub fn start_logging(processes_to_observe: Vec<ProcessToObserve>) -> anyhow::Result<StopHandle> {
    let metrics_log = MetricsLog::new();
    let metrics_log_mutex = Mutex::new(metrics_log);
    let shared_metrics_log = Arc::new(metrics_log_mutex);

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

    // create a new cancellation token
    let token = CancellationToken::new();

    // start threads to collect metrics
    let mut join_set = JoinSet::new();
    if !a.is_empty() {
        let token = token.clone();
        let shared_metrics_log = shared_metrics_log.clone();
        tracing::debug!("Spawning bare metal thread");
        join_set.spawn(async move {
            tracing::info!("Logging PIDs: {:?}", a);
            tokio::select! {
                _ = token.cancelled() => {}
                _ = bare_metal::keep_logging(
                        a,
                        shared_metrics_log,
                    ) => {}
            }
        });
    }

    if !b.is_empty() {
        let token = token.clone();
        let shared_metrics_log = shared_metrics_log.clone();

        join_set.spawn(async move {
            tracing::info!("Logging containers: {:?}", b);
            tokio::select! {
                            _ = token.cancelled() => {}
                            _ = docker::keep_logging(
                                    b,
            shared_metrics_log,
                                 ) => {}
                         }
        });
    }

    Ok(StopHandle::new(token, join_set, shared_metrics_log))
}
