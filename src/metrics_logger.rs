/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

pub mod bare_metal;
pub mod docker;

use crate::{metrics::MetricsLog, ProcessToObserve};
use itertools::Itertools;
use std::sync::{Arc, Mutex};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

pub struct StopHandle {
    token: CancellationToken,
    join_set: JoinSet<()>,
    shared_metrics_log: Arc<Mutex<MetricsLog>>,
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
pub fn start_logging(processes_to_observe: &[ProcessToObserve]) -> anyhow::Result<StopHandle> {
    let metrics_log = MetricsLog::new();
    let metrics_log_mutex = Mutex::new(metrics_log);
    let shared_metrics_log = Arc::new(metrics_log_mutex);

    // split processes into bare metal & docker processes
    let (pids, container_names): (Vec<_>, Vec<_>) =
        processes_to_observe
            .iter()
            .partition_map(|proc| match proc {
                ProcessToObserve::ProcId(id) => itertools::Either::Left(id),
                ProcessToObserve::ContainerName(name) => itertools::Either::Right(name.clone()),
            });

    println!("pids = {:?}", pids);

    // create a new cancellation token
    let token = CancellationToken::new();

    // start threads to collect metrics
    let mut join_set = JoinSet::new();
    if !pids.is_empty() {
        let token = token.clone();
        let shared_metrics_log = shared_metrics_log.clone();

        join_set.spawn(async move {
            println!("spawned bare metal logger");
            tokio::select! {
                _ = token.cancelled() => {}
                _ = bare_metal::keep_logging(
                        pids,
                        shared_metrics_log,
                    ) => {}
            }
        });
    }

    if !container_names.is_empty() {
        let token = token.clone();
        let shared_metrics_log = shared_metrics_log.clone();

        join_set.spawn(async move {
            tokio::select! {
                _ = token.cancelled() => {}
                _ = docker::keep_logging(
                        container_names,
                        shared_metrics_log,
                    ) => {}
            }
        });
    }

    Ok(StopHandle::new(token, join_set, shared_metrics_log))
}

/// Enters an infinite loop logging metrics for each process to the metrics log. This function is
/// intended to be used to log live environments which do not exit. If it is run on the main thread
/// it will block.
///
/// **WARNING**
///
/// This function should only be called from within a task that can execute it on another thread
/// otherwise it will block the main thread completely.
///
/// This function is intended to used in conjunction with other code that will periodically save
/// and flush the metrics log it writes to.
///
/// # Arguments
///
/// * `processes` - The processes to observe in the live environment
/// * `metrics_log` - A log of all observed metrics. Another thread should periodically save and
/// flush this shared log.
///
/// # Returns
///
/// This function does not return, it requires that it's thread is cancelled.
pub async fn log_live(
    _processes_to_observe: Vec<ProcessToObserve>,
    _metrics_log: Arc<Mutex<MetricsLog>>,
) {
    // TODO: This should do exactly what log_scenario does but it should save the shared_metrics_log
    // at regular fixed intervals (either space or time)
    todo!("implement this!")
}
