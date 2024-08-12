CREATE TABLE IF NOT EXISTS metrics (
    run_id TEXT NOT NULL,
    process_id TEXT NOT NULL,
    process_name TEXT NOT NULL,
    cpu_usage DOUBLE PRECISION NOT NULL,
    cpu_total_usage DOUBLE PRECISION NOT NULL,
    cpu_core_count INTEGER NOT NULL,
    time_stamp BIGINT NOT NULL,
    PRIMARY KEY (run_id, process_id, time_stamp),
    FOREIGN KEY (run_id) REFERENCES run (id)
);
