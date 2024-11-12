pub mod daemon;
pub mod execution_plan;
pub mod live_monitor;
pub mod process_control;
pub mod scenario_runner;

use crate::config::Scenario;

#[derive(Debug)]
pub enum ExecutionMode<'a> {
    Live,
    Observation(Vec<&'a Scenario>),
    Daemon,
}
