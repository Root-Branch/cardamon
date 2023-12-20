CREATE TABLE IF NOT EXISTS scenario (
    id INTEGER NOT NULL PRIMARY KEY,
    cardamon_run_type TEXT NOT NULL,
    cardamon_run_id TEXT NOT NULL,
    scenario_name TEXT NOT NULL,
    start_time DATETIME NOT NULL,
    stop_time DATETIME NOT NULL
);