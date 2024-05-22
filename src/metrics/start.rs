use crate::metrics::docker::get_docker_stats;

use super::types::{BareInput::*, CPUError, CPUType::*};
use super::{bare::*, types::*};
pub async fn get_metrics(t: CPUType) -> anyhow::Result<CPUStatus, CPUError> {
    let result = match t {
        BareStats(s) => match s {
            ProcessID(id) => get_stats_pid(id).await,
            ProcessName(name) => get_stats_name(&name).await,
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
    #[derive(Debug, serde::Deserialize)]
    struct MyObj {
        prop_a: String,
    }

    #[test]
    fn serde_test() -> anyhow::Result<()> {
        let json_str = r#"{ "prop_a": "hello", "prop_b": 42 }"#;
        let obj: MyObj = serde_json::from_str(json_str).unwrap();
        println!("{obj:?}");

        assert!(true);

        Ok(())
    }
}
