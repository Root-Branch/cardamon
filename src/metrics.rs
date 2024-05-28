/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

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
}
