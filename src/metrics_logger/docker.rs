use crate::metrics::{CpuMetrics, MetricsLog};
use bollard::container::{ListContainersOptions, Stats, StatsOptions};
use bollard::Docker;
use chrono::Utc;
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
/// * `container_names` - The names of the containers to observe
/// * `metrics_log` - A log of all observed metrics. Another thread should periodically save and
/// flush this shared log.
///
/// # Returns
///
/// This function does not return, it requires that its thread is cancelled.
pub async fn keep_logging(_container_names: Vec<String>, metrics_log: Arc<Mutex<MetricsLog>>) {
    let docker = match Docker::connect_with_local_defaults() {
        Ok(docker) => docker,
        Err(e) => {
            eprintln!("Failed to connect to Docker: {}", e);
            return;
        }
    };

    loop {
        let mut filter = HashMap::new();
        filter.insert(String::from("status"), vec![String::from("running")]);
        let containers = match docker
            .list_containers(Some(ListContainersOptions {
                all: true,
                filters: filter,
                ..Default::default()
            }))
            .await
        {
            Ok(containers) => containers,
            Err(e) => {
                eprintln!("Failed to list containers: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
        };

        if containers.is_empty() {
            eprintln!("No running containers");
        } else {
            for container in containers {
                let container_id = container.id.as_ref().unwrap();
                let mut stream = docker.stats(
                    container_id,
                    Some(StatsOptions {
                        stream: false,
                        ..Default::default()
                    }),
                );

                if let Some(stats) = stream.next().await {
                    match stats {
                        Ok(stats) => {
                            let cpu_metrics =
                                calculate_cpu_metrics(container_id, &container.names, &stats);
                            let mut log = metrics_log.lock().unwrap();
                            log.push_metrics(cpu_metrics);
                        }
                        Err(e) => {
                            eprintln!("Error getting stats for container {}: {}", container_id, e);
                            let mut log = metrics_log.lock().unwrap();
                            log.push_error(anyhow::anyhow!(
                                "Error getting stats for container {}: {}",
                                container_id,
                                e
                            ));
                        }
                    }
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

#[cfg(target_os = "linux")]
fn calculate_cpu_metrics(
    container_id: &str,
    container_names: &Option<Vec<String>>,
    stats: &Stats,
) -> CpuMetrics {
    let cpu_delta =
        stats.cpu_stats.cpu_usage.total_usage - stats.precpu_stats.cpu_usage.total_usage;
    let system_delta = stats.cpu_stats.system_cpu_usage.unwrap_or(0)
        - stats.precpu_stats.system_cpu_usage.unwrap_or(0);
    let cpu_usage = if system_delta > 0 {
        (cpu_delta as f64 / system_delta as f64)
            * 100.0
            * stats.cpu_stats.online_cpus.unwrap_or(1) as f64
    } else {
        0.0
    };

    CpuMetrics {
        process_id: container_id.to_string(),
        process_name: container_names
            .as_ref()
            .and_then(|names| names.first())
            .cloned()
            .unwrap_or_default(),
        cpu_usage,
        core_count: stats.cpu_stats.online_cpus.unwrap_or(1) as i32,
        timestamp: Utc::now().timestamp(),
    }
}

#[cfg(target_os = "windows")]
fn calculate_cpu_metrics(
    container_id: &str,
    container_names: &Option<Vec<String>>,
    stats: &Stats,
) -> CpuMetrics {
    let cpu_delta =
        stats.cpu_stats.cpu_usage.total_usage - stats.precpu_stats.cpu_usage.total_usage;
    let cpu_usage = if stats.cpu_stats.cpu_usage.percpu_usage.is_some() {
        let cores = stats
            .cpu_stats
            .cpu_usage
            .percpu_usage
            .as_ref()
            .unwrap()
            .len();
        (cpu_delta as f64 / cores as f64) * 100.0
    } else {
        0.0
    };

    CpuMetrics {
        process_id: container_id.to_string(),
        process_name: container_names
            .as_ref()
            .and_then(|names| names.first())
            .cloned()
            .unwrap_or_default(),
        cpu_usage,
        core_count: stats.cpu_stats.online_cpus.unwrap_or(1) as i32,
        timestamp: Utc::now().timestamp(),
    }
}

#[cfg(test)]
mod tests {}
