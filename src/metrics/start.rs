use crate::metrics::docker::get_docker_stats;

use super::types::{BareInput::*, CPUError, CPUType::*};
use super::{bare::*, types::*};

pub async fn get_metrics(t: CPUType) -> anyhow::Result<CPUStatus, CPUError> {
    let result = match t {
        BareStats(s) => match s {
            ProcessID(id) => get_stats_pid(id).await,
            ProcessName(name) => get_stats_name(name).await,
        },
        DockerStats(s) => get_docker_stats(s).await,
        KuberetesStats(_s) => {
            unimplemented!("Getting Kubernetes stats is not implemented yet")
        }
    };
    // println!("CPU status: {:?}", result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::container::{
        Config, CreateContainerOptions, ListContainersOptions, StartContainerOptions,
    };
    use bollard::models::HostConfig;
    use bollard::Docker;
    use futures_util::stream::StreamExt;
    use std::process;
    use uuid::Uuid;

    #[tokio::test]
    async fn bare_metal_valid_pid() {
        assert!(get_metrics(BareStats(ProcessID(process::id())))
            .await
            .is_ok())
    }
    #[tokio::test]
    async fn bare_metal_invalid_pid() {
        assert!(!get_metrics(BareStats(ProcessID(std::u32::MAX)))
            .await
            .is_ok());
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

    async fn create_container(
        docker: &Docker,
        image_name: &str,
        container_name: &str,
        port: Option<u16>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Create tiny-image (Just an HTTP server, could be any program that stays up)
        let mut image_stream = docker.create_image(
            Some(bollard::image::CreateImageOptions {
                from_image: image_name,
                ..Default::default()
            }),
            None,
            None,
        );
        while let Some(info) = image_stream.next().await {
            if let Err(err) = info {
                eprintln!("Error creating image: {}", err);
                break;
            }
        }

        // Run the HTTP server (image is created)
        let mut container_config = Config {
            image: Some(image_name.to_string()),
            cmd: Some(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "python -m http.server 80".to_string(),
            ]),
            host_config: Some(HostConfig {
                auto_remove: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };

        if let Some(port) = port {
            container_config.cmd = Some(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                format!("python -m http.server {}", port),
            ]);
            container_config.exposed_ports = Some(std::collections::HashMap::from([(
                format!("{}/tcp", port),
                std::collections::HashMap::new(),
            )]));
            container_config.host_config = Some(HostConfig {
                auto_remove: Some(true),
                port_bindings: Some(std::collections::HashMap::from([(
                    format!("{}/tcp", port),
                    Some(vec![bollard::models::PortBinding {
                        host_ip: Some("0.0.0.0".to_string()),
                        host_port: Some(port.to_string()),
                    }]),
                )])),
                ..Default::default()
            });
        }

        let create_options = CreateContainerOptions {
            name: container_name.to_string(),
            ..Default::default()
        };

        // Create a container with the specified name
        let container = docker
            .create_container(Some(create_options), container_config)
            .await?;

        // Start the container
        docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await?;
        Ok(container.id)
    }
    async fn cleanup_cardamon_containers(container_name: &str) {
        let docker = Docker::connect_with_defaults().unwrap();
        let container_list = docker
            .list_containers(None::<ListContainersOptions<String>>)
            .await
            .unwrap();

        for container in container_list {
            if let Some(names) = container.names {
                for name in names {
                    if name.starts_with(container_name) {
                        let container_id = container.id.clone().unwrap();
                        docker
                            .remove_container(
                                &container_id,
                                Some(bollard::container::RemoveContainerOptions {
                                    force: true,
                                    ..Default::default()
                                }),
                            )
                            .await
                            .unwrap();

                        // Wait for the container to be removed
                        docker
                            .wait_container(
                                &container_id,
                                None::<bollard::container::WaitContainerOptions<String>>,
                            )
                            .collect::<Vec<_>>()
                            .await;
                    }
                }
            }
        }
    }
}
