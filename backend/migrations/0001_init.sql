CREATE TYPE benchmark_category AS ENUM (
    'inference_performance',
    'resource_efficiency',
    'model_quality',
    'agentic',
    'context_scalability'
);

CREATE TYPE run_status AS ENUM ('queued', 'running', 'completed', 'failed');

CREATE TABLE models (
    id              UUID PRIMARY KEY,
    name            TEXT        NOT NULL,
    file_path       TEXT        NOT NULL UNIQUE,
    architecture    TEXT,
    quantization    TEXT,
    size_label      TEXT,
    block_count     INTEGER,
    file_size_bytes BIGINT      NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE hardware_profiles (
    id               UUID PRIMARY KEY,
    cpu              TEXT        NOT NULL,
    gpu              TEXT,
    vram_total_bytes BIGINT,
    ram_total_bytes  BIGINT      NOT NULL,
    backend          TEXT        NOT NULL,
    driver_version   TEXT,
    fingerprint      TEXT        NOT NULL UNIQUE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE benchmark_runs (
    id             UUID PRIMARY KEY,
    model_id       UUID               NOT NULL REFERENCES models (id),
    hardware_id    UUID               NOT NULL REFERENCES hardware_profiles (id),
    category       benchmark_category NOT NULL,
    benchmark_name TEXT               NOT NULL,
    status         run_status         NOT NULL DEFAULT 'queued',
    config         JSONB              NOT NULL DEFAULT '{}'::jsonb,
    details        JSONB,
    error          TEXT,
    started_at     TIMESTAMPTZ,
    finished_at    TIMESTAMPTZ,
    created_at     TIMESTAMPTZ        NOT NULL DEFAULT now()
);

CREATE INDEX benchmark_runs_model_category_idx ON benchmark_runs (model_id, category);
CREATE INDEX benchmark_runs_benchmark_name_idx ON benchmark_runs (benchmark_name);

CREATE TABLE metrics (
    id     BIGSERIAL PRIMARY KEY,
    run_id UUID             NOT NULL REFERENCES benchmark_runs (id) ON DELETE CASCADE,
    name   TEXT             NOT NULL,
    value  DOUBLE PRECISION NOT NULL,
    unit   TEXT             NOT NULL,
    UNIQUE (run_id, name)
);

CREATE INDEX metrics_name_idx ON metrics (name);
