CREATE TABLE IF NOT EXISTS cpu_metrics (
    run_id TEXT NOT NULL,
    process_id TEXT NOT NULL,
    process_name TEXT NOT NULL,
    cpu_usage DOUBLE NOT NULL,
    total_usage DOUBLE NOT NULL,
    core_count INTEGER NOT NULL,
    timestamp BIGINT NOT NULL,
    PRIMARY KEY (run_id, process_id, timestamp)
);
