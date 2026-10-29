-- Allow 'cancelled' as a run status. SQLite can't alter a CHECK constraint, so both run tables
-- are rebuilt. `metrics` is rebuilt too: dropping `benchmark_runs` while `metrics` still
-- referenced it would cascade-delete every metric. Renaming `benchmark_runs_new` afterwards
-- re-points `metrics_new`'s foreign key at `benchmark_runs`.

CREATE TABLE benchmark_runs_new (
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
                       CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled')),
    config         TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(config)),
    details        TEXT CHECK (details IS NULL OR json_valid(details)),
    error          TEXT,
    started_at     TEXT,
    finished_at    TEXT,
    created_at     TEXT NOT NULL
);

INSERT INTO benchmark_runs_new
SELECT id, model_id, hardware_id, category, benchmark_name, status, config, details, error,
       started_at, finished_at, created_at
FROM benchmark_runs;

CREATE TABLE metrics_new (
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT    NOT NULL REFERENCES benchmark_runs_new (id) ON DELETE CASCADE,
    name   TEXT    NOT NULL,
    value  REAL    NOT NULL,
    unit   TEXT    NOT NULL,
    UNIQUE (run_id, name)
);

INSERT INTO metrics_new (id, run_id, name, value, unit)
SELECT id, run_id, name, value, unit FROM metrics;

DROP TABLE metrics;
DROP TABLE benchmark_runs;

ALTER TABLE benchmark_runs_new RENAME TO benchmark_runs;
ALTER TABLE metrics_new RENAME TO metrics;

CREATE INDEX benchmark_runs_model_category_idx ON benchmark_runs (model_id, category);
CREATE INDEX benchmark_runs_benchmark_name_idx ON benchmark_runs (benchmark_name);
CREATE INDEX metrics_name_idx ON metrics (name);
