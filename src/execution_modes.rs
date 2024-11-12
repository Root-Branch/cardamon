pub mod daemon;
pub mod live_monitor;
pub mod scenario_runner;

use crate::config::Scenario;

#[derive(Debug)]
pub enum ExecutionMode<'a> {
    Live,
    Observation(Vec<&'a Scenario>),
    Daemon,
}
