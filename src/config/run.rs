/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

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
