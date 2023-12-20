CREATE TABLE IF NOT EXISTS cpu_metrics (
    id INTEGER NOT NULL PRIMARY KEY,
    cardamon_run_type TEXT NOT NULL,
    cardamon_run_id TEXT NOT NULL,
    container_id TEXT NOT NULL,
    container_name TEXT NOT NULL,
    throttling_periods BIGINT NOT NULL,
    throttling_throttled_periods BIGINT NOT NULL,
    throttling_throttled_time BIGINT NOT NULL,
    usage_in_kernelmode BIGINT NOT NULL,
    usage_in_usermode BIGINT NOT NULL,
    usage_percent DOUBLE NOT NULL,
    usage_system BIGINT NOT NULL,
    usage_total BIGINT NOT NULL,
    timestamp DATETIME NOT NULL
);