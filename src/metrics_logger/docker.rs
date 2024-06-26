use crate::metrics::{CpuMetrics, MetricsLog};
use bollard::container::{ListContainersOptions, Stats, StatsOptions};
use bollard::Docker;
use chrono::Utc;
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};

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
pub async fn keep_logging(container_names: Vec<String>, metrics_log: Arc<Mutex<MetricsLog>>) {
    // This connects with system defaults, socket for unix, http for windows
    let docker = match Docker::connect_with_defaults() {
        Ok(docker) => {
            debug!("Successfully connected to Docker");
            docker
        }
        Err(e) => {
            error!("Failed to connect to Docker: {}", e);
            return;
        }
    };

    loop {
        let mut filter = HashMap::new();
        // Only running containers, we re-try in a second if the container is not running yet
        filter.insert(String::from("status"), vec![String::from("running")]);
        filter.insert(String::from("name"), container_names.clone());
        debug!("Listing containers with filter: {:?}", filter);
        let containers = match docker
            .list_containers(Some(ListContainersOptions {
                all: true,
                filters: filter,
                ..Default::default()
            }))
            .await
        {
            Ok(containers) => {
                debug!(
                    "Successfully listed containers. Count: {}",
                    containers.len()
                );
                containers
            }
            Err(e) => {
                error!("Failed to list containers: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
        };
        // Wait 1s and re-try, this is not an error, containers take a while to spin up
        if containers.is_empty() {
            warn!("No running containers");
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            continue;
        }
        for container in containers {
            if let Some(container_id) = container.id.as_ref() {
                let container_name_with_slash: String = container
                    .names
                    .clone()
                    .and_then(|names| names.first().cloned())
                    .unwrap_or_else(|| "unknown".to_string());
                let container_name = &container_name_with_slash[1..container_name_with_slash.len()]; // Container name "test" would be "/test" here, remove first char
                match docker
                    .stats(
                        container_id,
                        Some(StatsOptions {
                            stream: false,
                            ..Default::default()
                        }),
                    )
                    .next()
                    .await
                {
                    Some(Ok(stats)) => {
                        let cpu_metrics =
                            calculate_cpu_metrics(container_id, container_name.to_string(), &stats);
                        debug!(
                            "Pushing metrics to metrics log form container name/s {:?}",
                            container.names
                        );
                        metrics_log.lock().unwrap().push_metrics(cpu_metrics);
                        debug!("Logged metrics for container {}", container_id);
                    }
                    Some(Err(e)) => {
                        error!("Error getting stats for container {}: {}", container_id, e);
                        metrics_log.lock().unwrap().push_error(anyhow::anyhow!(
                            "Error getting stats for container {}: {}",
                            container_id,
                            e
                        ));
                    }
                    None => {
                        error!("No stats received for container {}", container_id);
                    }
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

fn calculate_cpu_metrics(container_id: &str, container_name: String, stats: &Stats) -> CpuMetrics {
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

    info!(
        "Calculated CPU metrics for container {} ({}), cpu percentage: {}",
        container_id, container_name, cpu_usage
    );

    CpuMetrics {
        process_id: container_id.to_string(),
        process_name: container_name,
        cpu_usage,
        core_count: stats.cpu_stats.online_cpus.unwrap_or(1) as i32,
        timestamp: Utc::now().timestamp_millis(),
    }
}
pub async fn get_container_status(container_name: &str) -> anyhow::Result<String> {
    let docker = Docker::connect_with_defaults().map_err(|e| {
        error!("Failed to connect to Docker: {}", e);
        anyhow::anyhow!("Failed to connect to Docker: {}", e)
    })?;

    debug!("Successfully connected to Docker");

    let mut filter = HashMap::new();
    filter.insert(String::from("name"), vec![container_name.to_string()]);

    debug!("Listing containers with filter: {:?}", filter);

    let containers = docker
        .list_containers(Some(ListContainersOptions {
            all: true,
            filters: filter,
            ..Default::default()
        }))
        .await
        .map_err(|e| {
            error!("Failed to list containers: {}", e);
            anyhow::anyhow!("Failed to list containers: {}", e)
        })?;

    debug!(
        "Successfully listed containers. Count: {}",
        containers.len()
    );

    if containers.is_empty() {
        return Ok(String::from("not_found"));
    }

    let container = &containers[0];
    let status = container.state.as_deref().unwrap_or("unknown").to_string();
    debug!("Container '{}' status: {}", container_name, status);

    Ok(status)
}

#[cfg(test)]
mod tests {}
