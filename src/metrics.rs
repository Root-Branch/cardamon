/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::data_access;

#[derive(Debug)]
pub struct MetricsLog {
    log: Vec<CpuMetrics>,
    err: Vec<anyhow::Error>,
}
impl MetricsLog {
    pub fn new() -> Self {
        Self {
            log: vec![],
            err: vec![],
        }
    }

    pub fn push_metrics(&mut self, metrics: CpuMetrics) {
        self.log.push(metrics);
    }

    pub fn push_error(&mut self, err: anyhow::Error) {
        self.err.push(err);
    }

    pub fn get_metrics(&self) -> &Vec<CpuMetrics> {
        &self.log
    }

    pub fn get_errors(&self) -> &Vec<anyhow::Error> {
        &self.err
    }

    pub fn has_errors(&self) -> bool {
        !self.err.is_empty()
    }
}
impl Default for MetricsLog {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct CpuMetrics {
    pub process_id: String,
    pub process_name: String,
    pub cpu_usage: f64,
    pub core_count: i32,
    pub timestamp: i64,
}
impl CpuMetrics {
    pub fn into_data_access(&self, cardamon_run_id: &str) -> data_access::cpu_metrics::CpuMetrics {
        data_access::cpu_metrics::CpuMetrics::new(
            cardamon_run_id,
            &self.process_id,
            &self.process_name,
            self.cpu_usage,
            0_f64,
            self.core_count as i64,
            self.timestamp,
        )
    }
}
