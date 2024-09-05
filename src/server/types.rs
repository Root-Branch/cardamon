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
    pub scenario_data: Vec<ScenarioDataResponse>,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioDataResponse {
    pub scenario_name: String,
    pub run_data: Vec<RunDataResponse>,
    pub trend: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDataResponse {
    pub run_id: i32,
    pub run_pow: f64,
    pub run_co2: f64,
    pub proc_data: Vec<ProcessDataResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessDataResponse {
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
