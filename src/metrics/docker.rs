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
                .map(|(current, previous)| current - previous);

            // If we don't return 0 values, it appears to return "inf" psosiblity due to diving by
            // zero on line where we do
            //let cpu_usage = (cpu_delta as f64 / system_delta as f64) * number_cpus * 100.0;
            let system_delta = match system_delta {
                Some(delta) => delta,
                None => {
                    return Ok(CPUStatus {
                        stats: vec![Stat {
                            id: container_id,
                            name: container_name,
                            total_usage: 0.0,
                            usage_by_process: 0.0,
                            core_count: stats_response.cpu_stats.online_cpus.unwrap_or(0) as f32,
                        }],
                    });
                }
            };
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
#[cfg(test)]
mod tests {
    use crate::metrics::common::*;
    use crate::metrics::start::get_metrics;

    use super::*;
    // Tests related to docker.rs
    // ...

    use bollard::container::ListContainersOptions;
    use bollard::Docker;
    use uuid::Uuid;

    use crate::metrics::types::CPUType::*;
    #[tokio::test]
    async fn docker_invalid_container_name() {
        let docker = Docker::connect_with_defaults().unwrap();
        // Need to check it's an invalid container name
        let mut container_name = format!("cardamon-test-container-{}", Uuid::new_v4().to_string());

        let container_list = docker
            .list_containers(None::<ListContainersOptions<String>>)
            .await
            .unwrap();
        // Create another if it exists
        while container_list.iter().any(|container| {
            container.names.as_ref().map_or(false, |names| {
                names
                    .iter()
                    .any(|name| name == &format!("/{}", container_name))
            })
        }) {
            container_name = format!("cardamon-test-container-{}", Uuid::new_v4().to_string());
        }

        let result = get_metrics(DockerStats(DockerType::ContainerName(
            container_name.to_string(),
        )))
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn docker_valid_container_name() {
        let docker = Docker::connect_with_defaults().unwrap();
        let container_name = format!(
            "cardamon-test-valid-container-name-{}",
            Uuid::new_v4().to_string()
        );
        let image_name = "python:3-alpine";

        // Create and start the container
        let _container_id = create_container(&docker, image_name, &container_name, None)
            .await
            .unwrap();

        // Get metrics
        let result = get_metrics(DockerStats(DockerType::ContainerName(
            container_name.to_string(),
        )))
        .await;
        assert!(result.is_ok(), "get_metrics failed: {:?}", result.err());
        // Cleanup
        cleanup_cardamon_containers("/cardamon-test-valid-container-name-").await;
    }

    #[tokio::test]
    async fn test_invalid_container_name() {
        let invalid_container_name = "".to_string();

        // Get metrics using invalid container name
        assert!(get_metrics(DockerStats(DockerType::ContainerName(
            invalid_container_name
        )))
        .await
        .is_err());
    }

    #[tokio::test]
    async fn test_invalid_container_id() {
        let invalid_container_id = "".to_string();

        // Get metrics using invalid container ID
        assert!(
            get_metrics(DockerStats(DockerType::ContainerID(invalid_container_id)))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_valid_port() {
        let docker = Docker::connect_with_defaults().unwrap();
        let container_name = format!("cardamon-test-valid-port-{}", Uuid::new_v4().to_string());
        let image_name = "python:3-alpine";
        let mut port = 8000;

        // Iterate over ports until a free one is found
        while is_port_in_use(port).await {
            port += 1;
        }

        let _container_id = create_container(&docker, image_name, &container_name, Some(port))
            .await
            .unwrap();
        //Could be a one-liner, but easier to debug if we can print
        let result = get_metrics(DockerStats(DockerType::Port(port))).await;
        //println!("{:?}", result);
        assert!(result.is_ok());

        // Cleanup
        cleanup_cardamon_containers("/cardamon-test-valid-port-").await;
    }

    async fn is_port_in_use(port: u16) -> bool {
        use tokio::net::TcpListener;

        match TcpListener::bind(("0.0.0.0", port)).await {
            Ok(_) => false,
            Err(_) => true,
        }
    }
    #[tokio::test]
    async fn test_invalid_port() {
        let invalid_port = 9999;

        // Get metrics using invalid port
        assert!(get_metrics(DockerStats(DockerType::Port(invalid_port)))
            .await
            .is_err());
    }
}
