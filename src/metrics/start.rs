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
    use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
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
    async fn docker_valid_container_name() {
        let docker = Docker::connect_with_defaults().unwrap();
        let container_name = format!("card-test-container-{}", Uuid::new_v4().to_string());
        let image_name = "cpu_stress_test";
        let dockerfile_content = r#"FROM containerstack/alpine:latest
MAINTAINER Remon Lam [remon@containerstack.io]

RUN apk add --update --no-cache make wget gcc musl-dev linux-headers ca-certificates && \
    wget https://launchpad.net/ubuntu/+archive/primary/+files/stress-ng_0.03.12.orig.tar.gz && \
    tar -xzf stress-ng_0.03.12.orig.tar.gz && \
    cd stress*/ && \
    make install && \
    apk del make gcc musl-dev linux-headers ca-certificates && \
    rm -rf stress-ng_0.03.12.orig.tar.gz

ENTRYPOINT ["/usr/bin/stress-ng"]
CMD ["--help"]"#;
        let mut build_options = bollard::image::BuildImageOptions::default();
        build_options.t = image_name.to_string();
        let mut image_build_stream = docker.build_image(
            build_options,
            None,
            Some(dockerfile_content.as_bytes().to_vec().into()),
        );
        while let Some(msg) = image_build_stream.next().await {
            println!("Message: {msg:?}");
        }
        let container_config = Config {
            image: Some(container_name.clone()),
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
        let container = docker
            .create_container(Some(create_options), container_config)
            .await
            .unwrap();

        docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await
            .unwrap();

        assert!(get_metrics(DockerStats(DockerType::ContainerName(
            container_name.to_string()
        )))
        .await
        .is_ok());

        docker
            .remove_container(
                &container.id,
                Some(bollard::container::RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
            .unwrap();
    }
}
