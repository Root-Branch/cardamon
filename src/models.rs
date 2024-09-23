use std::{future::Future, pin::Pin};

use crate::{data::Data, entities::metrics::Model as Metrics};
use itertools::Itertools;
use sea_orm::DatabaseConnection;

pub type BoxFuture = Pin<Box<dyn Future<Output = anyhow::Result<Data>> + Send>>;

pub fn rab_linear_model(ci_g_w: f32) -> impl Fn(&Vec<&Metrics>, f32) -> Data {
    return move |metrics, cpu_avg_pow_w| {
        // TODO: THIS MUST BE FETCH ASYNCRONOUSLY USING THE run_id!

        let data = metrics
            .into_iter()
            .sorted_by(|a, b| b.time_stamp.cmp(&a.time_stamp))
            .tuple_windows()
            .map(|(a, b)| {
                // taking the midpoint of the two datapoints and dividing by 50 because we're
                // assuming avg_cpu_pow is at 50% utilization
                (0.5 * (a.cpu_usage + b.cpu_usage)) / 50_f64
                    * cpu_avg_pow_w as f64
                    * ((a.time_stamp - b.time_stamp) as f64 / 1000_f64)
            })
            .collect_vec();

        let pow_w = data.iter().fold(0_f64, |x, acc| x + acc);
        let co2_g_w = pow_w * ci_g_w as f64;

        Data {
            pow: pow_w,
            co2: co2_g_w,
        }
    };
}

pub fn rab_nonlinear_model(
    _ci: f32,
    _db: &DatabaseConnection,
) -> impl Fn(&Vec<&Metrics>, f32) -> Data {
    return move |_metrics, _run_id| todo!();
}
