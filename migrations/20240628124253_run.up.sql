CREATE TABLE IF NOT EXISTS run (
    id TEXT NOT NULL,
    start_time BIGINT NOT NULL,
    stop_time BIGINT,
    PRIMARY KEY (id)
)
