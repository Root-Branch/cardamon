/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use chrono::NaiveDateTime;

use serde::{Deserialize, Serialize};

// SCENARIOS
// ############################################################################
#[derive(Deserialize, Serialize)]
pub struct Scenario {
    pub cardamon_run_type: String,
    pub cardamon_run_id: String,
    pub scenario_name: String,
    pub iteration: u32,
    pub start_time: i64,
    pub stop_time: i64,
}

#[derive(Deserialize, Serialize)]
pub struct ScenarioSummaryOpts {
    pub scenarios: Vec<String>,
    pub last_n: u8,
    pub cpu_tdp: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProcessStats {
    pub process_name: String,
    pub energy_consumption_w: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RunStats {
    pub start_time: NaiveDateTime,
    pub process_stats: Vec<ProcessStats>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ScenarioRunStats {
    pub scenario_name: String,
    pub run_stats: Vec<RunStats>,
}

// DOCKER FIELDS & TAGS
// ############################################################################
#[derive(Deserialize)]
pub struct DockerContainerCpuFields {
    pub container_id: String,
    pub throttling_periods: i64,
    pub throttling_throttled_periods: i64,
    pub throttling_throttled_time: i64,
    pub usage_in_kernelmode: i64,
    pub usage_in_usermode: i64,
    pub usage_percent: f64,
    pub usage_system: i64,
    pub usage_total: i64,
}

#[derive(Deserialize)]
pub struct DockerContainerCpuTags {
    pub container_name: String,
    pub cardamon_run_type: String,
    pub cardamon_run_id: String,
}

#[derive(Deserialize)]
pub struct DockerContainerMemFields {
    pub container_id: String,
    pub active_anon: i64,
    pub active_file: i64,
    pub inactive_anon: i64,
    pub inactive_file: i64,
    pub limit: i64,
    pub max_usage: i64,
    pub pgfault: i64,
    pub pgmajfault: i64,
    pub unevictable: i64,
    pub usage: i64,
    pub usage_percent: f64,
}

#[derive(Deserialize)]
pub struct DockerContainerMemTags {
    pub container_name: String,
    pub cardamon_run_type: String,
    pub cardamon_run_id: String,
}

// METRICS AND BATCHED METRICS
// ############################################################################
#[derive(Deserialize)]
#[serde(tag = "name")]
pub enum Metrics {
    #[serde(rename = "docker_container_cpu")]
    DockerContainerCpu {
        timestamp: i64,
        fields: DockerContainerCpuFields,
        tags: DockerContainerCpuTags,
    },

    #[serde(rename = "docker_container_mem")]
    DockerContainerMem {
        timestamp: i64,
        fields: DockerContainerMemFields,
        tags: DockerContainerMemTags,
    },
}

#[derive(Deserialize)]
pub struct Batch {
    pub metrics: Vec<Metrics>,
}
