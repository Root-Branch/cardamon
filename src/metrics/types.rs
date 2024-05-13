use std::error::Error;
use std::fmt;

pub enum CPUType {
    BareStats(BareInput),
    DockerStats(DockerType),
    KuberetesStats(KubernetesInput),
}
pub enum DockerType {
    ContainerName(String),
    ContainerID(String),
    ImageName(String),
}

pub enum BareInput {
    ProcessID(u32),
    ProcessName(String),
}
pub struct KubernetesInput {}

#[derive(Debug)]
pub struct CPUStatus {
    pub stats: Vec<Stat>,
}

#[derive(Debug)]
pub struct Stat {
    pub id: String,
    pub name: String,
    // Needs to be divided by core count ( Bare metal)
    pub usage_by_process: f32,
    // Does NOT need to be divided by core count ( Bare metal )
    pub total_usage: f32,
    pub core_count: f32,
}
#[derive(Debug)]
pub enum CPUError {
    ProcessNotFound(String),
    DockerBollardError(bollard::errors::Error),
}
impl fmt::Display for CPUError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CPUError::ProcessNotFound(pid) => write!(f, "Process not found: {}", pid),
            CPUError::DockerBollardError(e) => write!(f, "Docker error: {}", e),
        }
    }
}
impl From<bollard::errors::Error> for CPUError {
    fn from(value: bollard::errors::Error) -> Self {
        CPUError::DockerBollardError(value)
    }
}
impl Error for CPUError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            _ => None,
        }
    }
}
