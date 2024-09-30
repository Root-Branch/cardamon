use crate::config::ProcessToObserve;
use crate::metrics::{CpuMetrics, MetricsLog};
use bollard::container::{ListContainersOptions, Stats, StatsOptions};
use bollard::Docker;
use chrono::Utc;
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, warn};

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
///                   flush this shared log.
///
/// # Returns
///
/// This function does not return, it requires that its thread is cancelled.
pub async fn keep_logging(
    procs_to_observe: Vec<ProcessToObserve>,
    metrics_log: Arc<Mutex<MetricsLog>>,
) {
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

    let mut container_names = vec![];
    for proc_to_observe in procs_to_observe.into_iter() {
        match proc_to_observe {
            ProcessToObserve::ManagedContainers {
                process_name: _,
                container_names: names,
                down: _,
            } => {
                container_names.append(&mut names.clone());
            }

            ProcessToObserve::ExternalContainers(names) => {
                container_names.append(&mut names.clone())
            }

            _ => panic!("wat!"),
        }
    }

    // Only running containers, we re-try in a second if the container is not running yet
    let mut filter = HashMap::new();
    filter.insert(String::from("status"), vec![String::from("running")]);
    filter.insert(String::from("name"), container_names.clone());
    debug!("Listing containers with filter: {:?}", filter);

    let container_list = docker
        .list_containers(Some(ListContainersOptions {
            all: true,
            filters: filter,
            ..Default::default()
        }))
        .await;

    let containers = match container_list {
        Ok(containers) => {
            debug!(
                "Successfully listed containers. Count: {}",
                containers.len()
            );
            containers
        }
        Err(e) => {
            error!("Failed to list containers: {}", e);
            return;
            // tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            // continue;
        }
    };

    // Wait 1s and re-try, this is not an error, containers take a while to spin up
    if containers.is_empty() {
        warn!("No running containers");
        return;
        // tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        // continue;
    }

    loop {
        for container in &containers {
            if let Some(container_id) = container.id.as_ref() {
                let container_name_with_slash = container
                    .names
                    .clone()
                    .and_then(|names| names.first().cloned())
                    .unwrap_or_else(|| "unknown".to_string());
                let container_name = &container_name_with_slash[1..container_name_with_slash.len()]; // Container name "test" would be "/test" here, remove first char

                let docker_stats = docker
                    .stats(
                        container_id,
                        Some(StatsOptions {
                            stream: false,
                            ..Default::default()
                        }),
                    )
                    .next()
                    .await;

                match docker_stats {
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
    }
}

fn calculate_cpu_metrics(container_id: &str, container_name: String, stats: &Stats) -> CpuMetrics {
    let core_count = stats.cpu_stats.online_cpus.unwrap_or(0);
    let cpu_delta =
        stats.cpu_stats.cpu_usage.total_usage - stats.precpu_stats.cpu_usage.total_usage;
    let system_delta = stats.cpu_stats.system_cpu_usage.unwrap_or(0)
        - stats.precpu_stats.system_cpu_usage.unwrap_or(0);
    let cpu_usage = if system_delta > 0 {
        (cpu_delta as f64 / system_delta as f64) * core_count as f64
    } else {
        0.0
    };
    debug!(
        "Calculated CPU metrics for container {} ({}), cpu percentage: {}",
        container_id, container_name, cpu_usage
    );
    CpuMetrics {
        process_id: container_id.to_string(),
        process_name: container_name,
        cpu_usage,
        core_count: core_count as i32,
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
mod tests {
    use crate::{
        config::ProcessToObserve,
        metrics::{CpuMetrics, MetricsLog},
        metrics_logger::{
            docker::{get_container_status, keep_logging},
            StopHandle,
        },
    };
    use bollard::{
        container::{Config, CreateContainerOptions, RemoveContainerOptions},
        image::{BuildImageOptions, RemoveImageOptions},
        Docker,
    };
    use bytes::Bytes;
    use chrono::Utc;
    use core::time;
    use futures_util::StreamExt;
    use nanoid::nanoid;
    use std::{
        io::Cursor,
        sync::{Arc, Mutex},
    };
    use tar::{Builder, Header};
    use tokio::{task::JoinSet, time::sleep};
    use tokio_util::sync::CancellationToken;

    async fn create_and_start_container(docker: &Docker) -> (String, String, String) {
        // container_id,
        // container_name
        // image_id
        // Smallest image I can create that doesn't exit ( 4.2mb), alpine is 7 ish
        let dockerfile = r#"
FROM busybox
CMD ["sleep", "infinity"]
"#;

        // Bollard has 2 options for creating an image
        // 1 - Dockerfile from *remote* url
        // 2 - Dockerfile from *tar file*
        // We'll create an in-memory tar file and use this
        // We want the bytes of the tar file for building
        let tar_bytes = {
            // Create a buffer to hold tar archive data
            let mut tar_buffer = Vec::new();
            // Use a nested block as we want to explicityly end the borrow of tar_buffer by
            // tar_builder
            {
                // Create a builder that'll write to our buffer
                let mut tar_builder = Builder::new(&mut tar_buffer);
                // Gnu format header, set path of file, size & permissions
                let mut header = Header::new_gnu();
                header.set_path("Dockerfile").unwrap();
                header.set_size(dockerfile.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                // Append to builder
                tar_builder
                    .append(&header, Cursor::new(dockerfile))
                    .unwrap();
                // Write to tar_buffer
                tar_builder.finish().unwrap();
            }
            // return bytes ( wanted by bollard::build_image
            Bytes::from(tar_buffer)
        };
        // Nano generates them with random from A-Z ) Plus _ and -
        // 2.. Removes _ and - as these are invalid
        let image_id = nanoid!(10, &nanoid::alphabet::SAFE[2..]).to_lowercase();
        let image_id_latest = format!("{}:latest", image_id);
        // Build the image
        let options = BuildImageOptions {
            dockerfile: "Dockerfile",
            t: &image_id_latest,
            ..Default::default()
        };
        // build image
        let mut build_stream = docker.build_image(options, None, Some(tar_bytes));
        // Docker streams the build process of making an image, meaning you can stop half-way if
        // something is wrong / you want a timeout for example.
        // In this case we want to continue until there's no more
        while let Some(output) = build_stream.next().await {
            output.unwrap();
        }
        // Create and start the container
        let container_name = format!(
            "cardamon-test-container-{}",
            nanoid!(10, &nanoid::alphabet::SAFE[2..]).to_lowercase()
        );
        let container = docker
            .create_container(
                Some(CreateContainerOptions {
                    name: container_name.as_str(),
                    ..Default::default()
                }),
                Config {
                    image: Some(image_id_latest),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        docker
            .start_container::<String>(&container.id, None)
            .await
            .unwrap();

        (container.id, container_name, image_id)
    }

    async fn cleanup_container(docker: &Docker, container_id: &str, image_id: &str) {
        // CLEANUP
        // We could "stop" container then "remove" container, but remove + force does this for us
        // ( Plus it sets the "grace" period docker has to 0, immediately stopping it )
        docker
            .remove_container(
                container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    v: true,
                    link: false,
                }),
            )
            .await
            .unwrap();

        docker
            .remove_image(
                image_id,
                Some(RemoveImageOptions {
                    force: true,
                    noprune: false,
                }),
                None,
            )
            .await
            .unwrap();
    }

    #[test]
    fn test_metrics_log() {
        let mut log = MetricsLog::new();

        let metrics = CpuMetrics {
            process_id: "123".to_string(),
            process_name: "test".to_string(),
            cpu_usage: 50.0,
            core_count: 4,
            timestamp: Utc::now().timestamp_millis(),
        };

        log.push_metrics(metrics);
        assert_eq!(log.get_metrics().len(), 1);

        log.push_error(anyhow::anyhow!("Error here"));
        assert!(log.has_errors());
        assert_eq!(log.get_errors().len(), 1);
    }

    #[tokio::test]
    async fn test_container_status() {
        // Test container status with a tiny container
        // Connect with system defaults ( socket on unix, http on windows )
        let docker = Docker::connect_with_local_defaults().unwrap();
        let (container_id, container_name, image_id) = create_and_start_container(&docker).await;

        // Test get_container_status
        let status = get_container_status(&container_name).await.unwrap();
        assert_eq!(status, "running", "Container should be in 'running' state");
        cleanup_container(&docker, &container_id, &image_id).await;
    }

    #[tokio::test]
    async fn test_keep_logging() {
        // pub async fn keep_logging(container_names: Vec<String>, metrics_log: Arc<Mutex<MetricsLog>>) {
        // Create a metrics log
        let metrics_log = MetricsLog::new();

        // Wrap it in a mutex ( enabling lock + unlock avoiding race condition )
        let metrics_log_mutex = Mutex::new(metrics_log);

        // Wrap in arc ( smart pointer, allows multiple mutable references )
        let shared_metrics_log = Arc::new(metrics_log_mutex);

        // Connect to docker
        let docker = Docker::connect_with_local_defaults().unwrap();

        // Create empty container
        let (container_id, container_name, image_id) = create_and_start_container(&docker).await;

        // Token to "cancel" keep logging
        let token = CancellationToken::new();

        // Allows for joining of multiple tasks, used because we have both bare-metal and docker
        // This joinset will have 1 item, so normally you wouldn't use one in this case
        // But this is a test so :shrug:
        let mut join_set = JoinSet::new();

        // Clone these values before moving them into the spawned task
        let task_token = token.clone();
        let task_metrics_log = shared_metrics_log.clone();
        let task_container_name = container_name.clone();

        let proc_to_observe = ProcessToObserve::ManagedContainers {
            process_name: "".to_string(),
            container_names: vec![task_container_name],
            down: Some("".to_string()),
        };

        // Spawn task ( async )
        join_set.spawn(async move {
            tokio::select! {
                _ = task_token.cancelled() => {}
                _ = keep_logging(vec![proc_to_observe], task_metrics_log)=> {}
            }
        });

        // Create stop handle ( used to extract metrics log and cancel )
        let stop_handle = StopHandle::new(token, join_set, shared_metrics_log);

        // Wait for period of time ( to get logs)
        sleep(time::Duration::new(2, 0)).await;

        // Stop logging and get metrics_logs from keep_logging()
        let metrics_log = stop_handle.stop().await.unwrap();

        // Should have no errors & some metrics
        assert!(!metrics_log.has_errors());
        assert!(!metrics_log.get_metrics().is_empty());
        assert_eq!(
            container_name,
            metrics_log.get_metrics().first().unwrap().process_name
        );

        // Cleanup
        cleanup_container(&docker, &container_id, &image_id).await;
    }
}
