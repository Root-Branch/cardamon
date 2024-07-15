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
    pub todo: String,
}

// Single scenario
#[derive(Debug, Deserialize, Serialize)]
pub struct ScenarioResponse {
    pub todo: String,
}
