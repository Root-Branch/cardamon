use super::types::{CPUError, CPUStatus, DockerType, Stat};
use bollard::{container::ListContainersOptions, Docker};
use futures_util::TryStreamExt;
use std::{collections::HashMap, time::Duration};
use tokio::time::sleep;
// cpu_usage = (cpu_delta / system_delta) * number_cpus * 100.0
// Delta is calculated via the previous stats, docker records this
// If it's our first check, we have to check twice
pub async fn get_docker_stats(docker_type: DockerType) -> Result<CPUStatus, CPUError> {
    let docker = Docker::connect_with_defaults().map_err(CPUError::DockerBollardError)?;
    let mut filter = HashMap::new();
    filter.insert(String::from("status"), vec![String::from("running")]);

    let containers = &docker
        .list_containers(Some(ListContainersOptions {
            all: true,
            filters: filter,
            ..Default::default()
        }))
        .await
        .map_err(CPUError::DockerBollardError)?;

    let mut stats = Vec::new();

    for container in containers {
        let container_name = container.names.as_ref().unwrap()[0].clone();
        let container_id = container.id.as_ref().unwrap().clone();
        let image_name = container.image.as_ref().unwrap().clone();

        let matches = match &docker_type {
            DockerType::ContainerName(name) => format!("/{}", *name) == container_name,
            DockerType::ContainerID(id) => container_id == *id,
            DockerType::ImageName(image) => image_name == *image,
        };

        if matches {
            let stats_response = docker
                .stats(&container.id.as_ref().unwrap(), None)
                .try_next()
                .await
                .map_err(CPUError::DockerBollardError)?
                .ok_or_else(|| {
                    CPUError::ProcessNotFound(format!("container_id -> {}", container_id))
                })?;

            let cpu_usage = if stats_response.precpu_stats.cpu_usage.total_usage == 0 {
                // If there are no pre-stats available, capture the stats again after a short delay
                sleep(Duration::from_secs(1)).await;
                let current_stats = docker
                    .stats(&container.id.as_ref().unwrap(), None)
                    .try_next()
                    .await
                    .map_err(CPUError::DockerBollardError)?
                    .ok_or_else(|| {
                        CPUError::ProcessNotFound(format!("container_id -> {}", container_id))
                    })?;

                let cpu_delta = current_stats.cpu_stats.cpu_usage.total_usage
                    - stats_response.cpu_stats.cpu_usage.total_usage;
                let system_delta = current_stats
                    .cpu_stats
                    .system_cpu_usage
                    .zip(stats_response.cpu_stats.system_cpu_usage)
                    .map(|(current, previous)| current - previous)
                    .unwrap_or(0);
                let number_cpus = current_stats.cpu_stats.online_cpus.unwrap_or(0) as f64;
                (cpu_delta as f64 / system_delta as f64) * number_cpus * 100.0
            } else {
                // If pre-stats are available, use them for calculation
                let cpu_delta = stats_response.cpu_stats.cpu_usage.total_usage
                    - stats_response.precpu_stats.cpu_usage.total_usage;
                let system_delta = stats_response
                    .cpu_stats
                    .system_cpu_usage
                    .zip(stats_response.precpu_stats.system_cpu_usage)
                    .map(|(current, previous)| current - previous)
                    .unwrap_or(0);
                let number_cpus = stats_response.cpu_stats.online_cpus.unwrap_or(0) as f64;
                (cpu_delta as f64 / system_delta as f64) * number_cpus * 100.0
            };

            let stat = Stat {
                id: container_id,
                name: container_name,
                usage_by_process: cpu_usage as f32,
                total_usage: cpu_usage as f32,
                core_count: stats_response.cpu_stats.online_cpus.unwrap_or(0) as f32,
            };

            stats.push(stat);
        }
    }

    if stats.is_empty() {
        match docker_type {
            DockerType::ContainerName(name) => Err(CPUError::ContainerNameNotFound(format!(
                "container_name -> {}",
                name
            ))),
            DockerType::ContainerID(id) => Err(CPUError::ContainerIDNotFound(format!(
                "container_id -> {}",
                id
            ))),
            DockerType::ImageName(image) => Err(CPUError::ImageNameNotFound(format!(
                "image_name -> {}",
                image
            ))),
        }
    } else {
        Ok(CPUStatus { stats })
    }
}
