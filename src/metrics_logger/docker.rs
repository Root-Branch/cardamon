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
    let mut error_buffer: Vec<anyhow::Error> = vec![];
    let mut buffer: Vec<CpuMetrics> = vec![];
    let mut iteration_count = 0;
    let mut last_timestamp: HashMap<String, i64> = HashMap::new();

    // This *gets* cores + ensures cadvisor is online, will block loop if error
    let cores = loop {
        match cadvisor_machine(cad_port).await {
            Ok(i) => {
                info!("cadvisor ok!");
                break i;
            }
            Err(e) => {
                error!("cadvisor not online, error: {e}");

                // Push the error to the error buffer
                error_buffer.push(e);
                // Arc mutex locks hold until the value drops / end of code block
                // We're using {} to ensure we don't keep the metrics_log locked whilst waiting
                {
                    // Flush the error buffer to the metrics_log
                    let mut metrics_log = metrics_log
                        .lock()
                        .expect("Failed to acquire lock on MetricsLog");
                    for error in error_buffer.drain(..) {
                        metrics_log.push_error(error);
                    }
                }
                // Wait for a certain duration before retrying
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                info!("No longer sleeping");
            }
        }
    };

    loop {
        for (container_name, container_url) in urls.iter() {
            let metr = get_cadvisor_metrics(container_name, container_url, cores).await;
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
                    error!("Error with geting metrics {e}");
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
    info!("Checking cadvisor is online");
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
fn calculate_cpu_percent_unix(previous_cpu: u64, previous_system: u64, v: &CpuUsage) -> f64 {
    info!("Previous CPU: {previous_cpu} , Previous system: {previous_system}");
    let mut cpu_percent = 0.0;

    // calculate the change for the cpu usage of the container in between readings
    let cpu_delta = v.total as f64 - previous_cpu as f64;

    // calculate the change for the entire system between readings
    let system_delta = v.system as f64 - previous_system as f64;

    //let online_cpus = v.cpu_stats.online_cpus as f64;

    //if online_cpus == 0.0 {
    //let online_cpus = v.cpu_stats.cpu_usage.percpu_usage.len() as f64;
    //}
    info!("Cpu Delta: {cpu_delta}, System Delta {system_delta}");
    if system_delta > 0.0 && cpu_delta > 0.0 {
        //cpu_percent = (cpu_delta / system_delta) * online_cpus * 100.0;
        cpu_percent = (cpu_delta / system_delta) * 100.0;
    } else {
        error!("System delta or cpu delta are 0");
    }

    cpu_percent
}
async fn get_cadvisor_metrics(
    container_name: &String,
    cadvisor_url: &String,
    cores: i32,
) -> anyhow::Result<CpuMetrics> {
    info!("getting metrics from cadvisor, url: {cadvisor_url}");
    let response = reqwest::get(cadvisor_url).await?;
    let status_code = response.status();

    if status_code == reqwest::StatusCode::INTERNAL_SERVER_ERROR {
        let error_message = response.text().await.map_err(|e| {
            anyhow::anyhow!(
                "Failed to read error response body from cAdvisor URL: {}\nError: {}",
                cadvisor_url,
                e
            )
        })?;

        if error_message.contains("failed to get Docker container") {
            return Err(anyhow::anyhow!(
                "Container not found for cAdvisor URL: {}",
                cadvisor_url
            ));
        } else {
            return Err(anyhow::anyhow!(
                "Internal Server Error from cAdvisor URL: {}\nError: {}",
                cadvisor_url,
                error_message
            ));
        }
    }

    let body = response.text().await.map_err(|e| {
        anyhow::anyhow!(
            "Failed to read response body from cAdvisor URL: {}\nError: {}",
            cadvisor_url,
            e
        )
    })?;

    let parsed_body = serde_json::from_str::<CadvisorOutput>(&body).map_err(|e| {
        anyhow::anyhow!(
            "Failed to parse JSON response from cAdvisor URL: {}\nError: {}\nResponse Body: {}\nStatusCode: {}",
            cadvisor_url,
            e,
            body,
            status_code
        )
    })?;
    if parsed_body.containers.len() != 1 {
        return Err(anyhow::anyhow!(
            "Body containers length is not 1 it is {}",
            parsed_body.containers.len()
        ));
    }
    // Should only be one
    for (_, v) in parsed_body.containers.iter() {
        if let Some(latest_stats) = v.stats.last() {
            if let Some(latest_minus_one) = v.stats.get(v.stats.len() - 2) {
                debug!("latest stats:{:?}", latest_stats);
                let previous_cpu = latest_minus_one.cpu.usage.total;
                let previous_system = latest_minus_one.cpu.usage.system;
                // Taken from docker docs ( golang )
                /*
                func calculateCPUPercentUnix(previousCPU, previousSystem uint64, v *types.StatsJSON) float64 {
                    var (
                        cpuPercent = 0.0
                        // calculate the change for the cpu usage of the container in between readings
                        cpuDelta = float64(v.CPUStats.CPUUsage.TotalUsage) - float64(previousCPU)
                        // calculate the change for the entire system between readings
                        systemDelta = float64(v.CPUStats.SystemUsage) - float64(previousSystem)
                        onlineCPUs  = float64(v.CPUStats.OnlineCPUs)
                    )

                    if onlineCPUs == 0.0 {
                        onlineCPUs = float64(len(v.CPUStats.CPUUsage.PercpuUsage))
                    }
                    if systemDelta > 0.0 && cpuDelta > 0.0 {
                        cpuPercent = (cpuDelta / systemDelta) * onlineCPUs * 100.0
                    }
                    return cpuPercent
                }
                 */
                let cpu_percentage = calculate_cpu_percent_unix(
                    previous_cpu,
                    previous_system,
                    &latest_stats.cpu.usage,
                );
                info!("Cpu usage as a percent {cpu_percentage}");
                let dt = DateTime::parse_from_rfc3339(&latest_stats.timestamp)
                    .unwrap_or_default()
                    .timestamp_millis();
                debug!("Timestamp: {dt}");
                return Ok(CpuMetrics {
                    process_id: v.id.clone(),
                    process_name: container_name.to_string(),
                    cpu_usage: cpu_percentage as f64,
                    core_count: cores,
                    timestamp: dt,
                });
            } else {
                error!("No previous  ( -2 ) stats");
            }
        } else {
            error!("No previous stats");
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
    //name: String,
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
