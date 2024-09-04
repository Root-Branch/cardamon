use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ScenariosParams {
    #[serde(rename = "fromDate")]
    pub from_date: Option<i64>,
    #[serde(rename = "toDate")]
    pub to_date: Option<i64>,
    #[serde(rename = "searchQuery")]
    pub search_query: Option<String>,
    pub page: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenariosResponse {
    pub scenario_data: Vec<ScenarioData>,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioData {
    pub scenario_name: String,
    pub run_data: Vec<RunData>,
    pub trend: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunData {
    pub run_id: i32,
    pub run_pow: f64,
    pub run_co2: f64,
    pub proc_data: Vec<ProcessData>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessData {
    pub proc_id: String,
    pub pow: f64,
    pub co2: f64,
    pub pow_perc: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub current_page: u64,
    pub per_page: u64,
    pub total_pages: u64,
}
