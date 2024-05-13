use super::types::{CPUError, CPUStatus, DockerType, Stat};
use bollard::{container::ListContainersOptions, Docker};
use std::collections::HashMap;

pub async fn get_docker_stats(docker_type: DockerType) -> anyhow::Result<CPUStatus, CPUError> {
    let docker = Docker::connect_with_defaults().map_err(|e| CPUError::DockerBollardError(e))?;
    let mut filter = HashMap::new();
    filter.insert(String::from("status"), vec![String::from("running")]);
    let containers = &docker
        .list_containers(Some(ListContainersOptions {
            all: true,
            filters: filter,
            ..Default::default()
        }))
        .await
        .map_err(|e| CPUError::DockerBollardError(e))?;
    println!("{:?}", containers);
    todo!()
}
