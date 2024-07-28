use serde::{Deserialize, Serialize};

// Multiple scenarios
#[derive(Debug, Deserialize)]
pub struct ScenariosParams {
    #[serde(rename = "fromDate")]
    pub from_date: Option<i64>,
    #[serde(rename = "toDate")]
    pub to_date: Option<i64>,
    #[serde(rename = "searchQuery")]
    pub search_query: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

// Single scenario
#[derive(Debug, Deserialize)]
pub struct ScenarioParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

// Multiple scenarios
#[derive(Debug, Deserialize, Serialize)]
pub struct ScenarioRun {
    pub run_id: String,
    pub iterations: Vec<Iteration>,
}
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct Iteration {
    pub run_id: String,
    pub scenario_name: String,
    pub iteration: i64,
    pub start_time: i64,
    pub stop_time: i64,
    pub usage: Option<Vec<Usage>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    pub name: String,
    pub avg_co2_emission: f64,
    pub avg_cpu_utilization: f64,
    pub avg_power_consumption: f64,
    pub last_start_time: u64,
    pub co2_emission_trend: Vec<f64>,
    pub runs: Vec<ScenarioRun>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScenariosResponse {
    pub scenarios: Vec<Scenario>,
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub current_page: u32,
    pub total_pages: u32,
    pub per_page: u32,
    // DISTINCT scenarios, not total total
    pub total_scenarios: u32,
}

// Single scenario
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioResponse {
    pub scenario: Scenario,
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Runs {
    pub run_id: String,
    pub iteration: u32,
    pub start_time: i64,
    pub end_time: i64,
    pub co2_emission: f64,
    pub power_consumption: f64,
    pub cpu_utilization: Vec<CpuUtilization>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuUtilization {
    pub process_name: String,
    pub cpu_usage: Vec<Usage>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub cpu_usage: f64,
    pub timestamp: i64,
}
