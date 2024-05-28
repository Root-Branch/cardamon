/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

pub mod bare_metal;
pub mod docker;

use crate::{metrics::MetricsLog, scenario_runner, settings::Scenario};
use itertools::Itertools;
use std::sync::{Arc, Mutex};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

pub enum Process {
    BareMetal(bare_metal::BareMetalProcess),
    Docker(docker::DockerProcess),
}

/// Logs a single scenario run
///
/// # Arguments
///
/// * `scenario` - The scenario as described in the settings file
/// * `processes` - The processes you wish to observe during the scenario run
///
/// # Returns
///
/// A `Result` containing the metrics log for the given scenario or an `Error` if either
/// the scenario failed to complete successfully or any of the loggers contained errors.
pub async fn log_scenario(
    scenario: Scenario,
    processes: Vec<Process>,
) -> anyhow::Result<MetricsLog> {
    // TODO: This function also needs to return the scenario run

    let metrics_log = MetricsLog::new();
    let metrics_log_mutex = Mutex::new(metrics_log);
    let shared_metrics_log = Arc::new(metrics_log_mutex);

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
                _ = bare_metal::keep_logging(
                        bare_metal_procs,
                        shared_metrics_log,
                    ) => {}
            }
        });
    }

    if !docker_procs.is_empty() {
        let token = token.clone();
        let shared_metrics_log = shared_metrics_log.clone();

        join_set.spawn(async move {
            tokio::select! {
                _ = token.cancelled() => {}
                _ = docker::keep_logging(
                        docker_procs,
                        shared_metrics_log,
                    ) => {}
            }
        });
    }

    // run the scenario
    match scenario_runner::run_scenario(&scenario).await {
        Ok(_) => {
            tracing::info!("Scenario completed successfully");

            // cancel loggers
            token.cancel();
            loop {
                if join_set.join_next().await.is_none() {
                    break;
                }
            }

            // take ownership of metrics log
            let metrics_log = Arc::try_unwrap(shared_metrics_log)
                .expect("Mutex guarding metrics_log shouldn't have multiple owners!")
                .into_inner()
                .expect("Should be able to take ownership of metrics_log");

            // return error if metrics log contains any errors
            if metrics_log.has_errors() {
                return Err(anyhow::anyhow!(
                    "Metrics log contains errors, please check trace"
                ));
            }

            // TODO: Save scenario

            // TODO: Save metrics log

            Ok(metrics_log)
        }
        Err(e) => {
            // cancel loggers
            token.cancel();
            loop {
                if join_set.join_next().await.is_none() {
                    break;
                }
            }

            Err(anyhow::anyhow!("Scenario contains errors.\n{e}"))
        }
    }
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
pub async fn log_live(_processes: Vec<Process>, _metrics_log: Arc<Mutex<MetricsLog>>) {
    // TODO: This should do exactly what log_scenario does but it should save the shared_metrics_log
    // at regular fixed intervals (either space or time)
    todo!("implement this!")
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn log_scenario_should_return_metrics_log_without_errors() -> anyhow::Result<()> {
        // 1. Create a couple of processes and run them using the (not yet developed) app_runner
        //   1.1 This will return a list of Process that we can use when calling log_scenario
        //
        // 2. Create a Scenario
        //
        // 3. Log the scenario

        //
        // log_scenario(
        //     Scenario {
        //         name: "name".to_string(),
        //         iteration: 3,
        //         command: "sleep 5".to_string(),
        //     },
        //     vec![
        //         Process::BareMetal(bare_metal::BareMetalProcess::ProcId(42)),
        //         //Process::Docker(DockerProcess::ContainerId(String::from("my_container"))),
        //     ],
        // )
        // .await?;

        Ok(())
    }
}
