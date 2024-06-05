use chrono::DateTime;
use serde::Deserialize;
use tracing::{debug, error, info};

use crate::metrics::{CpuMetrics, MetricsLog};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// Enters an infinite loop logging metrics for each process to the metrics log. This function is
/// intended to be called from `metrics_logger::log_scenario` or `metrics_logger::log_live`
///
/// **WARNING**
///
/// This function should only be called from within a task that can execute it on another thread
/// otherwise it will block the main thread completely.
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
pub async fn keep_logging(container_names: Vec<String>, metrics_log: Arc<Mutex<MetricsLog>>) {
    // TODO replace with another port, default cadvisor port is 8080
    // Env variable ? ( we're currently not using dotenv)
    // 8080 will be used by 90% of web servers
    let cad_port = 8080;

    // Hashmap of container_name:cadvisor_url
    // prevents multiple calcualtions of same thing
    let urls: HashMap<String, String> = container_names
        .into_iter()
        .map(|container_name| {
            let url = format!(
                "http://127.0.0.1:{}/api/v1.3/docker/{}",
                cad_port, container_name
            );
            (container_name, url)
        })
        .collect();
    // This *gets* cores + ensures cadvisor is online, needs to block loop in future to abvoid
    // senseless cpu usage
    let cores = match cadvisor_machine(cad_port).await {
        Ok(i) => {
            info!("cadvisor ok!");
            i
        }
        Err(e) => {
            error!("cadvisor not ok {e}"); // Block the loop ?
            0
        }
    };
    let mut buffer: Vec<CpuMetrics> = vec![];
    let mut error_buffer: Vec<anyhow::Error> = vec![];
    let mut iteration_count = 0;
    let mut last_timestamp: HashMap<String, i64> = HashMap::new();

    loop {
        for (container_name, container_url) in urls.iter() {
            let metr = get_cadvisor_metrics(container_url, cores).await;
            match metr {
                Ok(metrics) => {
                    // If we're already checked this container, and we have a timestamp
                    if let Some(prev_timestamp) = last_timestamp.get(container_name) {
                        // If the timestamp is *not* the same as last time ( new metric)
                        if metrics.timestamp != *prev_timestamp {
                            // Then push the metric
                            debug!(
                                "New metrics for container ID {container_name}, saving metrics."
                            );
                            buffer.push(metrics.clone());
                            last_timestamp.insert(container_name.clone(), metrics.timestamp);
                        } // else, don't push it
                    } else {
                        // If there's *no* timestamp for this container name ( it's new)
                        debug!("New container_name, saving metrics");
                        // Push it
                        buffer.push(metrics.clone());
                        last_timestamp.insert(container_name.clone(), metrics.timestamp);
                    }
                }
                Err(e) => {
                    error_buffer.push(e);
                }
            }
        }

        iteration_count += 1;

        if iteration_count == 5 {
            let mut metrics_log = metrics_log
                .lock()
                .expect("Failed to acquire lock on MetricsLog");
            for metric in buffer.drain(..) {
                metrics_log.push_metrics(metric);
            }
            for error in error_buffer.drain(..) {
                metrics_log.push_error(error);
            }
            debug!("Flushed metrics and errors to MetricsLog");

            iteration_count = 0;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}
// This also serves to check cadvisor is *online*
async fn cadvisor_machine(port: u16) -> anyhow::Result<i32> {
    debug!("Checking cadvisor is online");
    let url = format!("http://127.0.0.1:{}/api/v1.3/machine", port);
    let response = reqwest::get(&url)
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to make request to cAdvisor URL: {}\nError: {}",
                url,
                e
            )
        })?
        .json::<CadvisorMachine>()
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse JSON response from cAdvisor URL: {}\nError: {}",
                url,
                e
            )
        })?;
    debug!("Got num_cores from cadvisor {:?}", response.num_cores);
    Ok(response.num_cores)
}
#[derive(Default, Debug, Deserialize)]
struct CadvisorMachine {
    num_cores: i32,
}

async fn get_cadvisor_metrics(cadvisor_url: &String, cores: i32) -> anyhow::Result<CpuMetrics> {
    debug!("getting metrics from cadvisor, url: {cadvisor_url}");
    let body = reqwest::get(cadvisor_url)
        .await?
        .json::<CadvisorOutput>()
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse JSON response from cAdvisor URL: {}\nError: {}",
                cadvisor_url,
                e
            )
        })?;
    if body.containers.len() != 1 {
        return Err(anyhow::anyhow!(
            "Body containers length is not 1 it is {}",
            body.containers.len()
        ));
    }
    // Should only be one
    for (_, v) in body.containers.iter() {
        if let Some(latest_stats) = v.stats.last() {
            debug!("latest stats:{:?}", latest_stats);
            let cpu_usage = latest_stats.cpu.usage.total;
            // Unix Ts
            let dt = DateTime::parse_from_rfc3339(&latest_stats.timestamp)
                .unwrap_or_default()
                .timestamp();
            return Ok(CpuMetrics {
                process_id: v.id.clone(),
                process_name: v.name.clone(),
                cpu_usage: cpu_usage as f64,
                core_count: cores,
                timestamp: dt,
            });
        }
    }
    Err(anyhow::anyhow!("No metrics gotten, "))
}

#[derive(Debug, Deserialize)]
struct CadvisorOutput {
    #[serde(flatten)]
    containers: std::collections::HashMap<String, ContainerInfo>,
}

#[derive(Debug, Deserialize)]
struct ContainerInfo {
    id: String,
    name: String,
    stats: Vec<ContainerStats>,
}

#[derive(Debug, Deserialize)]
struct ContainerStats {
    timestamp: String,
    cpu: CpuStats,
}

#[derive(Debug, Deserialize)]
struct CpuStats {
    usage: CpuUsage,
}

#[derive(Debug, Deserialize)]
struct CpuUsage {
    total: u64,
    #[allow(dead_code)]
    user: u64,
    #[allow(dead_code)]
    system: u64,
}

#[cfg(test)]
mod tests {}
