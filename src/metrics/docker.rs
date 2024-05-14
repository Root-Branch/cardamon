use super::types::{CPUError, CPUStatus, DockerType, Stat};
use bollard::{container::ListContainersOptions, Docker};
use futures_util::TryStreamExt;
use std::collections::HashMap;
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
        let matches = match &docker_type {
            //Container names are formatted
            DockerType::ContainerName(name) => format!("/{}", *name) == container_name,
            DockerType::ContainerID(id) => container_id == *id,
            DockerType::Port(port) => {
                if let Some(ports) = &container.ports {
                    ports.iter().any(|p| p.private_port == *port)
                } else {
                    false
                }
            }
        };

        if matches {
            let stats_response = docker
                .stats(&container.id.as_ref().unwrap(), None)
                .try_next()
                .await
                .map_err(CPUError::DockerBollardError)?
                .ok_or_else(|| {
                    CPUError::ProcessNotFound(format!(
                        "stat response failed container_id -> {}",
                        container_id
                    ))
                })?;

            let cpu_delta = stats_response.cpu_stats.cpu_usage.total_usage
                - stats_response.precpu_stats.cpu_usage.total_usage;
            let system_delta = stats_response
                .cpu_stats
                .system_cpu_usage
                .zip(stats_response.precpu_stats.system_cpu_usage)
                .map(|(current, previous)| current - previous)
                .unwrap_or(0);
            let number_cpus = stats_response.cpu_stats.online_cpus.unwrap_or(0) as f64;
            let cpu_usage = (cpu_delta as f64 / system_delta as f64) * number_cpus * 100.0;

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
            DockerType::Port(port) => Err(CPUError::PortNotFound(format!("port -> {}", port))),
        }
    } else {
        Ok(CPUStatus { stats })
    }
}
