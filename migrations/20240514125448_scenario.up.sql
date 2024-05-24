CREATE TABLE IF NOT EXISTS scenario (
    id TEXT NOT NULL PRIMARY KEY,
    cardamon_run_id TEXT NOT NULL,
    scenario_name TEXT NOT NULL,
    iteration INT NOT NULL,
    start_time BIGINT NOT NULL,
    stop_time BIGINT NOT NULL
);
