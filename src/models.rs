use std::{future::Future, pin::Pin};

use crate::{config::Power, data::Data, entities::metrics::Model as Metrics};
use itertools::Itertools;

pub type BoxFuture = Pin<Box<dyn Future<Output = anyhow::Result<Data>> + Send>>;

fn boa_model(a: f64, b: f64, c: f64, d: f64) -> impl Fn(f64) -> f64 {
    move |workload| a * (b * (workload + c)).ln() + d
}

pub fn rab_model(metrics: &Vec<&Metrics>, power: &Power, ci_g_wh: f64) -> Data {
    let data = metrics
        .iter()
        .sorted_by(|a, b| b.time_stamp.cmp(&a.time_stamp))
        .tuple_windows()
        .map(|(x, y)| {
            match *power {
                Power::Curve(a, b, c, d) => {
                    let cpu_util = 0.5 * (x.cpu_usage + y.cpu_usage) * 100.0;
                    let delta_t_h = (x.time_stamp - y.time_stamp) as f64 / 3_600_000.0;

                    // boa_model(a, b, c, d)(cpu_util * delta_t_h)
                    boa_model(a, b, c, d)(cpu_util) * delta_t_h
                }

                Power::Tdp(tdp) => {
                    let delta_t_h = (x.time_stamp - y.time_stamp) as f64 / 3_600_000.0;

                    // taking the midpoint of the two datapoints and dividing by 50 because we're
                    // assuming tdp is at 50% utilization
                    (0.5 * (x.cpu_usage + y.cpu_usage)) / 50.0 * tdp * delta_t_h
                }
            }
        })
        .collect_vec();

    let pow_w = data.iter().fold(0_f64, |x, acc| x + acc);
    let co2_g_wh = pow_w * ci_g_wh;

    Data {
        pow: pow_w,
        co2: co2_g_wh,
    }
}
