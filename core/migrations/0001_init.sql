-- SQLite schema. UUIDs are stored as hyphenated TEXT, timestamps as RFC 3339
-- TEXT written by the application (chrono), and JSON documents as TEXT.
-- Postgres enums become CHECK constraints over TEXT.

CREATE TABLE models (
    id              TEXT    PRIMARY KEY NOT NULL,
    name            TEXT    NOT NULL,
    file_path       TEXT    NOT NULL UNIQUE,
    architecture    TEXT,
    quantization    TEXT,
    size_label      TEXT,
    block_count     INTEGER,
    file_size_bytes INTEGER NOT NULL,
    created_at      TEXT    NOT NULL
);

CREATE TABLE hardware_profiles (
    id               TEXT    PRIMARY KEY NOT NULL,
    cpu              TEXT    NOT NULL,
    gpu              TEXT,
    vram_total_bytes INTEGER,
    ram_total_bytes  INTEGER NOT NULL,
    backend          TEXT    NOT NULL,
    driver_version   TEXT,
    fingerprint      TEXT    NOT NULL UNIQUE,
    created_at       TEXT    NOT NULL
);

CREATE TABLE benchmark_runs (
    id             TEXT PRIMARY KEY NOT NULL,
    model_id       TEXT NOT NULL REFERENCES models (id),
    hardware_id    TEXT NOT NULL REFERENCES hardware_profiles (id),
    category       TEXT NOT NULL CHECK (category IN (
                       'inference_performance',
                       'resource_efficiency',
                       'model_quality',
                       'agentic',
                       'context_scalability'
                   )),
    benchmark_name TEXT NOT NULL,
    status         TEXT NOT NULL DEFAULT 'queued'
                       CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    config         TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(config)),
    details        TEXT CHECK (details IS NULL OR json_valid(details)),
    error          TEXT,
    started_at     TEXT,
    finished_at    TEXT,
    created_at     TEXT NOT NULL
);

CREATE INDEX benchmark_runs_model_category_idx ON benchmark_runs (model_id, category);
CREATE INDEX benchmark_runs_benchmark_name_idx ON benchmark_runs (benchmark_name);

CREATE TABLE metrics (
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT    NOT NULL REFERENCES benchmark_runs (id) ON DELETE CASCADE,
    name   TEXT    NOT NULL,
    value  REAL    NOT NULL,
    unit   TEXT    NOT NULL,
    UNIQUE (run_id, name)
);

CREATE INDEX metrics_name_idx ON metrics (name);
