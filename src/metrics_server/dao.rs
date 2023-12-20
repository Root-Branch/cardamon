/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use chrono::NaiveDateTime;
use diesel::prelude::*;

// SCENARIO RUNS
// ############################################################################

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = crate::metrics_server::dao_schema::scenario)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Scenario {
    pub cardamon_run_type: String,
    pub cardamon_run_id: String,
    pub scenario_name: String,
    pub start_time: NaiveDateTime,
    pub stop_time: NaiveDateTime,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = crate::metrics_server::dao_schema::scenario)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct GetScenario {
    pub id: i32,
    #[diesel(embed)]
    pub scenario: Scenario,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::metrics_server::dao_schema::scenario)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NewScenario {
    #[diesel(embed)]
    pub scenario: Scenario,
}

// CPU METRICS
// ############################################################################

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = crate::metrics_server::dao_schema::cpu_metrics)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct CpuMetrics {
    pub cardamon_run_type: String,
    pub cardamon_run_id: String,
    pub container_id: String,
    pub container_name: String,
    pub throttling_periods: i64,
    pub throttling_throttled_periods: i64,
    pub throttling_throttled_time: i64,
    pub usage_in_kernelmode: i64,
    pub usage_in_usermode: i64,
    pub usage_percent: f64,
    pub usage_system: i64,
    pub usage_total: i64,
    pub timestamp: NaiveDateTime,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = crate::metrics_server::dao_schema::cpu_metrics)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct GetCpuMetrics {
    pub id: i32,
    #[diesel(embed)]
    pub metrics: CpuMetrics,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::metrics_server::dao_schema::cpu_metrics)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NewCpuMetrics {
    #[diesel(embed)]
    pub metrics: CpuMetrics,
}
