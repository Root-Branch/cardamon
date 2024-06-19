use serde::{Deserialize, Serialize};
use utoipa::{openapi::schema, IntoParams, ToSchema};

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct RunParams {
    #[schema(example = "2023-06-01T00:00:00.000Z")]
    pub start_date: Option<String>, // String of NaiveDateTime
    #[schema(example = "2023-06-30T23:59:59.000Z")]
    pub end_date: Option<String>, // String of NaiveDateTime
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunWithScenario {
    #[schema(example = json!([ { "metricType": "CO2", "type": "TOTAL", "value": 0.81 }, { "metricType": "POWER", "type": "TOTAL", "value": 1.23 }, { "metricType": "CPU", "type": "TOTAL", "value": 2.34 } ]))]
    pub metrics: Vec<Metric>,
    #[schema(example = "2023-06-15T10:30:00.000Z")]
    pub start_time: String,
    #[schema(example = "Scenario 1")]
    pub scenario_name: Option<String>,
    #[schema(example = "run_123")]
    pub id: String,
    #[schema(example = "2023-06-15T11:00:00.000Z")]
    pub end_time: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunWithScenarioResponse {
    #[schema(example = json!([
    {
      "id": "id",
      "scenarioName": "scenarioName",
      "startTime": "2000-01-23T04:56:07.000Z",
      "endTime": "2000-01-23T04:56:07.000Z",
      "metrics": [
        {
          "metricType": "CO2",
          "type": "TOTAL",
          "value": 0.81
        },
        {
          "metricType": "POWER",
          "type": "TOTAL",
          "value": 1.23
        },
        {
          "metricType": "CPU",
          "type": "TOTAL",
          "value": 2.34
        }
      ]
    },
    {
      "id": "id",
      "scenarioName": "scenarioName",
      "startTime": "2000-01-23T04:56:07.000Z",
      "endTime": "2000-01-23T04:56:07.000Z",
      "metrics": [
        {
          "metricType": "CO2",
          "type": "TOTAL",
          "value": 0.81
        },
        {
          "metricType": "POWER",
          "type": "TOTAL",
          "value": 1.23
        },
        {
          "metricType": "CPU",
          "type": "TOTAL",
          "value": 2.34
        }
      ]
    }
  ]))]
    pub data: Vec<RunWithScenario>,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunsResponse {
    #[schema(example = json!([
    {
      "metrics": [
        {
          "metricType": "CO2",
          "type": "TOTAL",
          "value": 0.81
        },
        {
          "metricType": "POWER",
          "type": "TOTAL",
          "value": 1.23
        },
        {
          "metricType": "CPU",
          "type": "TOTAL",
          "value": 2.34
        }
      ],
      "startTime": "2000-01-23T04:56:07.000Z",
      "id": "id",
      "endTime": "2000-01-23T04:56:07.000Z"
    },
    {
      "metrics": [
        {
          "metricType": "CO2",
          "type": "TOTAL",
          "value": 0.81
        },
        {
          "metricType": "POWER",
          "type": "TOTAL",
          "value": 1.23
        },
        {
          "metricType": "CPU",
          "type": "TOTAL",
          "value": 2.34
        }
      ],
      "startTime": "2000-01-23T04:56:07.000Z",
      "id": "id",
      "endTime": "2000-01-23T04:56:07.000Z"
    }
    ]))]
    pub data: Vec<Runs>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Runs {
    #[schema(example = json!([ { "metricType": "CO2", "type": "TOTAL", "value": 0.81 }, { "metricType": "POWER", "type": "TOTAL", "value": 1.23 }, { "metricType": "CPU", "type": "TOTAL", "value": 2.34 } ]))]
    pub metrics: Vec<Metric>,
    #[schema(example = "2023-06-15T10:30:00.000Z")]
    pub start_time: String,
    #[schema(example = "run_123")]
    pub id: String,
    #[schema(example = "2023-06-15T11:00:00.000Z")]
    pub end_time: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub enum MetricType {
    #[default]
    TOTAL,
    AVERAGE,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetricResponse {
    #[schema(example = json!([
        {
            "metricType": "CO2",
            "type": "TOTAL",
            "value": 0.81
        },
        {
            "metricType": "POWER",
            "type": "TOTAL",
            "value": 1.23
        },
        {
            "metricType": "CPU",
            "type": "TOTAL",
            "value": 2.34
        }
    ]))]
    metrics: Vec<Metric>,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Metric {
    #[schema(example = "CO2")]
    pub metric_type: String,
    #[serde(rename = "type", default)]
    pub type_field: MetricType,
    #[schema(example = 0.81)]
    pub value: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioResponse {
    #[schema(example = json!([
    {
      "iteration": 0,
      "startTime": "2000-01-23T04:56:07.000Z",
      "stopTime": "2000-01-23T04:56:07.000Z",
      "metrics": [
        {
          "metricType": "CO2",
          "type": "TOTAL",
          "value": 0.81
        },
        {
          "metricType": "POWER",
          "type": "TOTAL",
          "value": 1.23
        },
        {
          "metricType": "CPU",
          "type": "TOTAL",
          "value": 2.34
        }
      ]
    },
    {
      "iteration": 1,
      "startTime": "2000-01-23T04:56:07.000Z",
      "stopTime": "2000-01-23T04:56:07.000Z",
      "metrics": [
        {
          "metricType": "CO2",
          "type": "TOTAL",
          "value": 0.81
        },
        {
          "metricType": "POWER",
          "type": "TOTAL",
          "value": 1.23
        },
        {
          "metricType": "CPU",
          "type": "TOTAL",
          "value": 2.34
        }
      ]
    }
  ]))]
    pub data: Scenario,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    #[schema(example = 1)]
    pub iteration: i64,
    #[schema(example = "2023-06-15T10:30:00.000Z")]
    pub start_time: String,
    #[schema(example = "2023-06-15T11:00:00.000Z")]
    pub stop_time: String,
    #[schema(example = json!([ { "metricType": "CO2", "type": "TOTAL", "value": 0.81 }, { "metricType": "POWER", "type": "TOTAL", "value": 1.23 }, { "metricType": "CPU", "type": "TOTAL", "value": 2.34 } ]))]
    pub metrics: Vec<Metric>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct GetCpuMetricsParams {
    #[schema(example = "2023-06-01T00:00:00.000Z")]
    start_date: Option<String>, // String of NaiveDateTime
    #[schema(example = "2023-06-30T23:59:59.000Z")]
    end_date: Option<String>, // String of NaiveDateTime
    #[schema(example = "run_123")]
    run_id: Option<String>,
    #[schema(example = "scenario_456")]
    scenario_id: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct GetMetricsParams {
    #[schema(example = "2023-06-01T00:00:00.000Z")]
    start_date: Option<String>, // String of NaiveDateTime
    #[schema(example = "2023-06-30T23:59:59.000Z")]
    end_date: Option<String>, // String of NaiveDateTime
    #[schema(example = "run_123")]
    run_id: Option<String>,
    r#type: MetricType,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CpuMetricsResponse {
    #[schema(example = json!([
        {
            "cpuUsage": 75.6,
            "processId": "process_123",
            "processName": "example_process",
            "totalUsage": 80.2,
            "id": "cpu_metric_123",
            "coreCount": 4,
            "timestamp": "2023-06-15T10:35:00.000Z"
        },
        {
            "cpuUsage": 68.3,
            "processId": "process_456",
            "processName": "another_process",
            "totalUsage": 72.9,
            "id": "cpu_metric_456",
            "coreCount": 4,
            "timestamp": "2023-06-15T10:36:00.000Z"
        }
    ]))]
    pub cpu_metrics: Vec<CpuMetric>,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CpuMetric {
    #[schema(example = 75.6)]
    pub cpu_usage: f64,
    #[schema(example = "process_123")]
    pub process_id: String,
    #[schema(example = "example_process")]
    pub process_name: String,
    #[schema(example = 80.2)]
    pub total_usage: f64,
    #[schema(example = "cpu_metric_123")]
    pub id: String,
    #[schema(example = 4)]
    pub core_count: i64,
    #[schema(example = "2023-06-15T10:35:00.000Z")]
    pub timestamp: String,
}

