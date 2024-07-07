CREATE TABLE IF NOT EXISTS iteration (
    run_id TEXT NOT NULL,
    scenario_name TEXT NOT NULL,
    iteration INT NOT NULL,
    start_time BIGINT NOT NULL,
    stop_time BIGINT NOT NULL,
    PRIMARY KEY (run_id, scenario_name, iteration),
    FOREIGN KEY (run_id) REFERENCES run (id)
);
