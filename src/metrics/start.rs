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
    println!("CPU status: {:?}", result);
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
        cleanup_cardamon_containers().await;
    }
    #[tokio::test]
    async fn docker_valid_container_name() {
        //Create tiny-image ( Just a http server, could be any program that stays up
        let docker = Docker::connect_with_defaults().unwrap();
        let container_name = format!("cardamon-test-container-{}", Uuid::new_v4().to_string());
        let image_name = "python:3-alpine";

        // Creation is a stream of messages from docker
        let mut image_stream = docker.create_image(
            Some(bollard::image::CreateImageOptions {
                from_image: image_name,
                ..Default::default()
            }),
            None,
            None,
        );
        while let Some(info) = image_stream.next().await {
            match info {
                Ok(..) => (),
                Err(err) => {
                    eprintln!("Error creating image: {}", err);
                    break;
                }
            }
        }
        // Run the http-server ( image is created )
        let container_config = Config {
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
        let create_options = CreateContainerOptions {
            name: container_name.clone(),
            ..Default::default()
        };
        // Create a container with our name
        let container = docker
            .create_container(Some(create_options), container_config)
            .await
            .unwrap();

        // Start it
        docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await
            .unwrap();
        // Get metrics
        assert!(get_metrics(DockerStats(DockerType::ContainerName(
            container_name.to_string()
        )))
        .await
        .is_ok());

        // Cleanup
        cleanup_cardamon_containers().await;
    }
    async fn cleanup_cardamon_containers() {
        let docker = Docker::connect_with_defaults().unwrap();
        let container_list = docker
            .list_containers(None::<ListContainersOptions<String>>)
            .await
            .unwrap();

        for container in container_list {
            if let Some(names) = container.names {
                for name in names {
                    if name.starts_with("/cardamon-test-container-") {
                        docker
                            .remove_container(
                                &container.id.clone().unwrap(),
                                Some(bollard::container::RemoveContainerOptions {
                                    force: true,
                                    ..Default::default()
                                }),
                            )
                            .await
                            .unwrap();
                    }
                }
            }
        }
    }
}
