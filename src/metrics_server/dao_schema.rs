// @generated automatically by Diesel CLI.

diesel::table! {
    cpu_metrics (id) {
        id -> Integer,
        cardamon_run_type -> Text,
        cardamon_run_id -> Text,
        container_id -> Text,
        container_name -> Text,
        throttling_periods -> BigInt,
        throttling_throttled_periods -> BigInt,
        throttling_throttled_time -> BigInt,
        usage_in_kernelmode -> BigInt,
        usage_in_usermode -> BigInt,
        usage_percent -> Double,
        usage_system -> BigInt,
        usage_total -> BigInt,
        timestamp -> Timestamp,
    }
}

diesel::table! {
    scenario (id) {
        id -> Integer,
        cardamon_run_type -> Text,
        cardamon_run_id -> Text,
        scenario_name -> Text,
        start_time -> Timestamp,
        stop_time -> Timestamp,
    }
}

diesel::allow_tables_to_appear_in_same_query!(cpu_metrics, scenario,);
