use bollard::container::{
    Config, CreateContainerOptions, ListContainersOptions, StartContainerOptions,
};
use bollard::models::HostConfig;
use bollard::Docker;
use futures_util::stream::StreamExt;

pub async fn create_container(
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
pub async fn cleanup_cardamon_containers(container_name: &str) {
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
