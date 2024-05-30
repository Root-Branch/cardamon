use crate::metrics::{CpuMetrics, MetricsLog};
use std::sync::{Arc, Mutex};

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
/// * `processes` - The processes to observe in the live environment
/// * `metrics_log` - A log of all observed metrics. Another thread should periodically save and
/// flush this shared log.
///
/// # Returns
///
/// This function does not return, it requires that it's thread is cancelled.
pub async fn keep_logging(_container_names: Vec<String>, _metrics_log: Arc<Mutex<MetricsLog>>) {
    todo!()
    /*
    let mut buffer: Vec<CpuStats> = vec![];
    let mut i = 0;
    loop {
        // generate random number (this will be replaced by call to sysinfo)
        // TODO: replace 1338 with actual data
        buffer.push(1338);

        // if buffer is full then write to shared metrics log
        if i == 9 {
            let mut metrics_log = metrics_log.lock().expect("");
            metrics_log.append(&mut buffer);
            println!("hello from docker");

            i = 0;
            buffer.clear();
        } else {
            i += 1;
        }

        // simulate waiting for more metrics
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
        */
}

// mod common {
//     use bollard::container::{
//         Config, CreateContainerOptions, ListContainersOptions, StartContainerOptions,
//     };
//     use bollard::models::HostConfig;
//     use bollard::Docker;
//     use futures_util::stream::StreamExt;
//
//     pub async fn create_container(
//         docker: &Docker,
//         image_name: &str,
//         container_name: &str,
//         port: Option<u16>,
//     ) -> Result<String, Box<dyn std::error::Error>> {
//         // Create tiny-image (Just an HTTP server, could be any program that stays up)
//         let mut image_stream = docker.create_image(
//             Some(bollard::image::CreateImageOptions {
//                 from_image: image_name,
//                 ..Default::default()
//             }),
//             None,
//             None,
//         );
//         while let Some(info) = image_stream.next().await {
//             if let Err(err) = info {
//                 eprintln!("Error creating image: {}", err);
//                 break;
//             }
//         }
//
//         // Run the HTTP server (image is created)
//         let mut container_config = Config {
//             image: Some(image_name.to_string()),
//             cmd: Some(vec![
//                 "/bin/sh".to_string(),
//                 "-c".to_string(),
//                 "python -m http.server 80".to_string(),
//             ]),
//             host_config: Some(HostConfig {
//                 auto_remove: Some(true),
//                 ..Default::default()
//             }),
//             ..Default::default()
//         };
//
//         if let Some(port) = port {
//             container_config.cmd = Some(vec![
//                 "/bin/sh".to_string(),
//                 "-c".to_string(),
//                 format!("python -m http.server {}", port),
//             ]);
//             container_config.exposed_ports = Some(std::collections::HashMap::from([(
//                 format!("{}/tcp", port),
//                 std::collections::HashMap::new(),
//             )]));
//             container_config.host_config = Some(HostConfig {
//                 auto_remove: Some(true),
//                 port_bindings: Some(std::collections::HashMap::from([(
//                     format!("{}/tcp", port),
//                     Some(vec![bollard::models::PortBinding {
//                         host_ip: Some("0.0.0.0".to_string()),
//                         host_port: Some(port.to_string()),
//                     }]),
//                 )])),
//                 ..Default::default()
//             });
//         }
//
//         let create_options = CreateContainerOptions {
//             name: container_name.to_string(),
//             ..Default::default()
//         };
//
//         // Create a container with the specified name
//         let container = docker
//             .create_container(Some(create_options), container_config)
//             .await?;
//
//         // Start the container
//         docker
//             .start_container(&container.id, None::<StartContainerOptions<String>>)
//             .await?;
//         Ok(container.id)
//     }
//     pub async fn cleanup_cardamon_containers(container_name: &str) {
//         let docker = Docker::connect_with_defaults().unwrap();
//         let container_list = docker
//             .list_containers(None::<ListContainersOptions<String>>)
//             .await
//             .unwrap();
//
//         for container in container_list {
//             if let Some(names) = container.names {
//                 for name in names {
//                     if name.starts_with(container_name) {
//                         let container_id = container.id.clone().unwrap();
//                         docker
//                             .remove_container(
//                                 &container_id,
//                                 Some(bollard::container::RemoveContainerOptions {
//                                     force: true,
//                                     ..Default::default()
//                                 }),
//                             )
//                             .await
//                             .unwrap();
//
//                         // Wait for the container to be removed
//                         docker
//                             .wait_container(
//                                 &container_id,
//                                 None::<bollard::container::WaitContainerOptions<String>>,
//                             )
//                             .collect::<Vec<_>>()
//                             .await;
//                     }
//                 }
//             }
//         }
//     }
// }

// cpu_usage = (cpu_delta / system_delta) * number_cpus * 100.0
// Delta is calculated via the previous stats, docker records this
// If it's our first check, we have to check twice
async fn _get_metrics(_container_names: Vec<String>) -> anyhow::Result<CpuMetrics> {
    todo!()
    // let docker = Docker::connect_with_defaults().map_err(CPUError::DockerBollardError)?;
    // let mut filter = HashMap::new();
    // filter.insert(String::from("status"), vec![String::from("running")]);
    //
    // let containers = &docker
    //     .list_containers(Some(ListContainersOptions {
    //         all: true,
    //         filters: filter,
    //         ..Default::default()
    //     }))
    //     .await
    //     .map_err(CPUError::DockerBollardError)?;
    //
    // let mut stats = Vec::new();
    //
    // for container in containers {
    //     let container_name = container.names.as_ref().unwrap()[0].clone();
    //     let container_id = container.id.as_ref().unwrap().clone();
    //     let matches = match &docker_type {
    //         //Container names are formatted
    //         DockerType::ContainerName(name) => format!("/{}", *name) == container_name,
    //         DockerType::ContainerID(id) => container_id == *id,
    //         DockerType::Port(port) => {
    //             if let Some(ports) = &container.ports {
    //                 ports.iter().any(|p| p.private_port == *port)
    //             } else {
    //                 false
    //             }
    //         }
    //     };
    //
    //     if matches {
    //         let stats_response = docker
    //             .stats(&container.id.as_ref().unwrap(), None)
    //             .try_next()
    //             .await
    //             .map_err(CPUError::DockerBollardError)?
    //             .ok_or_else(|| {
    //                 CPUError::ProcessNotFound(format!(
    //                     "stat response failed container_id -> {}",
    //                     container_id
    //                 ))
    //             })?;
    //
    //         let cpu_delta = stats_response.cpu_stats.cpu_usage.total_usage
    //             - stats_response.precpu_stats.cpu_usage.total_usage;
    //         let system_delta = stats_response
    //             .cpu_stats
    //             .system_cpu_usage
    //             .zip(stats_response.precpu_stats.system_cpu_usage)
    //             .map(|(current, previous)| current - previous);
    //
    //         // If we don't return 0 values, it appears to return "inf" psosiblity due to diving by
    //         // zero on line where we do
    //         //let cpu_usage = (cpu_delta as f64 / system_delta as f64) * number_cpus * 100.0;
    //         let system_delta = match system_delta {
    //             Some(delta) => delta,
    //             None => {
    //                 return Ok(CPUStatus {
    //                     stats: vec![Stat {
    //                         id: container_id,
    //                         name: container_name,
    //                         total_usage: 0.0,
    //                         usage_by_process: 0.0,
    //                         core_count: stats_response.cpu_stats.online_cpus.unwrap_or(0) as f32,
    //                     }],
    //                 });
    //             }
    //         };
    //         let number_cpus = stats_response.cpu_stats.online_cpus.unwrap_or(0) as f64;
    //         let cpu_usage = (cpu_delta as f64 / system_delta as f64) * number_cpus * 100.0;
    //
    //         let stat = Stat {
    //             id: container_id,
    //             name: container_name,
    //             usage_by_process: cpu_usage as f32,
    //             total_usage: cpu_usage as f32,
    //             core_count: stats_response.cpu_stats.online_cpus.unwrap_or(0) as f32,
    //         };
    //
    //         stats.push(stat);
    //     }
    // }
    // if stats.is_empty() {
    //     match docker_type {
    //         DockerType::ContainerName(name) => Err(CPUError::ContainerNameNotFound(format!(
    //             "container_name -> {}",
    //             name
    //         ))),
    //         DockerType::ContainerID(id) => Err(CPUError::ContainerIDNotFound(format!(
    //             "container_id -> {}",
    //             id
    //         ))),
    //         DockerType::Port(port) => Err(CPUError::PortNotFound(format!("port -> {}", port))),
    //     }
    // } else {
    //     Ok(CPUStatus { stats })
    // }
}

#[cfg(test)]
mod tests {
    //     use crate::metrics::common::*;
    //     use crate::metrics::start::get_metrics;
    //
    //     use super::*;
    //     // Tests related to docker.rs
    //     // ...
    //
    //     use bollard::container::ListContainersOptions;
    //     use bollard::Docker;
    //     use uuid::Uuid;
    //
    //     use crate::metrics::types::CPUType::*;
    //     #[tokio::test]
    //     async fn docker_invalid_container_name() {
    //         let docker = Docker::connect_with_defaults().unwrap();
    //         // Need to check it's an invalid container name
    //         let mut container_name = format!("cardamon-test-container-{}", Uuid::new_v4().to_string());
    //
    //         let container_list = docker
    //             .list_containers(None::<ListContainersOptions<String>>)
    //             .await
    //             .unwrap();
    //         // Create another if it exists
    //         while container_list.iter().any(|container| {
    //             container.names.as_ref().map_or(false, |names| {
    //                 names
    //                     .iter()
    //                     .any(|name| name == &format!("/{}", container_name))
    //             })
    //         }) {
    //             container_name = format!("cardamon-test-container-{}", Uuid::new_v4().to_string());
    //         }
    //
    //         let result = get_metrics(DockerStats(DockerType::ContainerName(
    //             container_name.to_string(),
    //         )))
    //         .await;
    //
    //         assert!(result.is_err());
    //     }
    //
    //     #[tokio::test]
    //     async fn docker_valid_container_name() {
    //         let docker = Docker::connect_with_defaults().unwrap();
    //         let container_name = format!(
    //             "cardamon-test-valid-container-name-{}",
    //             Uuid::new_v4().to_string()
    //         );
    //         let image_name = "python:3-alpine";
    //
    //         // Create and start the container
    //         let _container_id = create_container(&docker, image_name, &container_name, None)
    //             .await
    //             .unwrap();
    //
    //         // Get metrics
    //         let result = get_metrics(DockerStats(DockerType::ContainerName(
    //             container_name.to_string(),
    //         )))
    //         .await;
    //         assert!(result.is_ok(), "get_metrics failed: {:?}", result.err());
    //         // Cleanup
    //         cleanup_cardamon_containers("/cardamon-test-valid-container-name-").await;
    //     }
    //
    //     #[tokio::test]
    //     async fn test_invalid_container_name() {
    //         let invalid_container_name = "".to_string();
    //
    //         // Get metrics using invalid container name
    //         assert!(get_metrics(DockerStats(DockerType::ContainerName(
    //             invalid_container_name
    //         )))
    //         .await
    //         .is_err());
    //     }
    //
    //     #[tokio::test]
    //     async fn test_invalid_container_id() {
    //         let invalid_container_id = "".to_string();
    //
    //         // Get metrics using invalid container ID
    //         assert!(
    //             get_metrics(DockerStats(DockerType::ContainerID(invalid_container_id)))
    //                 .await
    //                 .is_err()
    //         );
    //     }
    //
    //     #[tokio::test]
    //     async fn test_valid_port() {
    //         let docker = Docker::connect_with_defaults().unwrap();
    //         let container_name = format!("cardamon-test-valid-port-{}", Uuid::new_v4().to_string());
    //         let image_name = "python:3-alpine";
    //         let mut port = 8000;
    //
    //         // Iterate over ports until a free one is found
    //         while is_port_in_use(port).await {
    //             port += 1;
    //         }
    //
    //         let _container_id = create_container(&docker, image_name, &container_name, Some(port))
    //             .await
    //             .unwrap();
    //         //Could be a one-liner, but easier to debug if we can print
    //         let result = get_metrics(DockerStats(DockerType::Port(port))).await;
    //         //println!("{:?}", result);
    //         assert!(result.is_ok());
    //
    //         // Cleanup
    //         cleanup_cardamon_containers("/cardamon-test-valid-port-").await;
    //     }
    //
    //     async fn is_port_in_use(port: u16) -> bool {
    //         use tokio::net::TcpListener;
    //
    //         match TcpListener::bind(("0.0.0.0", port)).await {
    //             Ok(_) => false,
    //             Err(_) => true,
    //         }
    //     }
    //     #[tokio::test]
    //     async fn test_invalid_port() {
    //         let invalid_port = 9999;
    //
    //         // Get metrics using invalid port
    //         assert!(get_metrics(DockerStats(DockerType::Port(invalid_port)))
    //             .await
    //             .is_err());
    //     }
}
