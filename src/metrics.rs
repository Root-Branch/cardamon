use crate::entities::metrics;
use anyhow::anyhow;
use sea_orm::*;

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

    pub fn clear(&mut self) {
        self.log.clear();
    }

    pub async fn save(&self, run_id: i32, db: &DatabaseConnection) -> anyhow::Result<()> {
        // if metrics log contains errors then display them to the user and don't save anything
        if self.has_errors() {
            // log all the errors
            for err in &self.err {
                tracing::error!("{}", err);
            }
            return Err(anyhow!("Metric log contained errors, please see logs."));
        }

        for metrics in &self.log {
            metrics.into_active_model(run_id).save(db).await?;
        }

        Ok(())
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
    pub fn into_active_model(&self, run_id: i32) -> metrics::ActiveModel {
        metrics::ActiveModel {
            id: ActiveValue::NotSet,
            run_id: ActiveValue::Set(run_id),
            process_id: ActiveValue::Set(self.process_id.clone()),
            process_name: ActiveValue::Set(self.process_name.clone()),
            cpu_usage: ActiveValue::Set(self.cpu_usage),
            cpu_total_usage: ActiveValue::Set(0_f64),
            cpu_core_count: ActiveValue::Set(self.core_count),
            time_stamp: ActiveValue::Set(self.timestamp),
        }
    }
}
