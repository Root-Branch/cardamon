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
pub struct ScenariosResponse {
    pub scenarios: Vec<Scenario>,
    pub pagination: Pagination,
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
pub struct ScenarioResponse {
    pub todo: String,
}
