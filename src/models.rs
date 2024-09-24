use itertools::Itertools;

use crate::entities::metrics;

pub struct CardamonData {
    pub pow: f64,
    pub co2: f64,
}

pub fn rab_linear_model(
    data: Vec<&metrics::Model>,
    cpu_avg_pow_w: f64,
    ci_g_w: f64,
) -> CardamonData {
    let data = data
        .into_iter()
        .sorted_by(|a, b| b.time_stamp.cmp(&a.time_stamp))
        .tuples()
        .map(|(a, b)| {
            // taking the midpoint of the two datapoints and dividing by 50 because we're
            // assuming avg_cpu_pow is at 50% utilization
            (0.5 * (a.cpu_usage + b.cpu_usage)) / 50_f64
                * cpu_avg_pow_w
                * ((a.time_stamp - b.time_stamp) as f64 / 1000_f64)
        })
        .collect_vec();

    let pow_w = data.iter().fold(0_f64, |x, acc| x + acc);
    let co2_g_w = pow_w * ci_g_w;

    CardamonData {
        pow: pow_w,
        co2: co2_g_w,
    }
}

pub fn rab_nonlinear_model(
    _data: Vec<metrics::Model>,
    _cpu_pow_curve: f64,
    _ci: f64,
) -> CardamonData {
    todo!()
}
