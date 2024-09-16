pub mod dataset;
pub mod dataset_builder;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Data {
    pub pow: f64,
    pub co2: f64,
}
impl Default for Data {
    fn default() -> Self {
        Data {
            pow: 0_f64,
            co2: 0_f64,
        }
    }
}
impl std::ops::Add<&Data> for Data {
    type Output = Data;

    fn add(self, rhs: &Data) -> Data {
        Data {
            pow: self.pow + rhs.pow,
            co2: self.co2 + rhs.co2,
        }
    }
}
impl std::ops::Add<Data> for Data {
    type Output = Data;

    fn add(self, rhs: Data) -> Data {
        Data {
            pow: self.pow + rhs.pow,
            co2: self.co2 + rhs.co2,
        }
    }
}
impl Data {
    pub fn sum(data: &[&Data]) -> Self {
        data.into_iter()
            .fold(Data::default(), |acc, item| acc + *item)
    }

    pub fn mean(data: &[&Data]) -> Self {
        let len = data.len() as f64;
        let mut data = data
            .into_iter()
            .fold(Data::default(), |acc, item| acc + *item);

        data.pow /= len;
        data.co2 /= len;

        data
    }
}

#[derive(Debug, Serialize)]
pub struct ProcessData {
    pub process_id: String,
    pub data: Data,
    pub pow_perc: f64,
}

#[derive(Debug, Serialize)]
pub struct RunData {
    pub run_id: i32,
    pub start_time: i64,
    pub stop_time: i64,
    pub data: Data,
    pub process_data: Vec<ProcessData>,
}
impl RunData {
    pub fn duration(&self) -> f64 {
        (self.stop_time - self.start_time) as f64 / 1000.0
    }
}

#[derive(Debug, Serialize)]
pub struct ScenarioData {
    pub scenario_name: String,
    pub data: Data,
    pub run_data: Vec<RunData>,
    pub trend: f64,
}
