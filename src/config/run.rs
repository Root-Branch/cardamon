#[derive(Debug)]
pub enum ProcessToObserve {
    BareMetalId(u32),
    ContainerName(String),
}

#[derive(Debug)]
pub struct ScenarioToRun {
    pub name: String,
    pub command: String,
    pub iteration: u32,
}

#[derive(Debug)]
pub struct Run {
    pub processes_to_observe: Vec<ProcessToObserve>,
    pub scenarios_to_run: Vec<ScenarioToRun>,
}
