pub mod dataset;
pub mod dataset_builder;

#[derive(Debug)]
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

#[derive(Debug)]
pub struct ProcessData {
    pub process_id: String,
    pub data: Data,
}

#[derive(Debug)]
pub struct RunData {
    pub run_id: i32,
    pub data: Data,
    pub process_data: Vec<ProcessData>,
}

#[derive(Debug)]
pub struct ScenarioData {
    pub scenario_name: String,
    pub data: Data,
    pub run_data: Vec<RunData>,
}
