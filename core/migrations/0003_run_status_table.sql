-- Run statuses live in a lookup table instead of a CHECK, so adding one is an INSERT, not a
-- table rebuild. ALTER TABLE can only add a REFERENCES column that defaults to NULL, so the
-- new column is backfilled, the old CHECKed column dropped, and NOT NULL enforced by triggers.

CREATE TABLE run_statuses (
    name TEXT PRIMARY KEY NOT NULL
);

INSERT INTO run_statuses (name) VALUES
    ('queued'), ('running'), ('completed'), ('failed'), ('cancelled');

ALTER TABLE benchmark_runs ADD COLUMN run_status TEXT REFERENCES run_statuses (name);
UPDATE benchmark_runs SET run_status = status;
ALTER TABLE benchmark_runs DROP COLUMN status;
ALTER TABLE benchmark_runs RENAME COLUMN run_status TO status;

CREATE TRIGGER benchmark_runs_status_insert_not_null
BEFORE INSERT ON benchmark_runs
WHEN NEW.status IS NULL
BEGIN
    SELECT RAISE(ABORT, 'NOT NULL constraint failed: benchmark_runs.status');
END;

CREATE TRIGGER benchmark_runs_status_update_not_null
BEFORE UPDATE OF status ON benchmark_runs
WHEN NEW.status IS NULL
BEGIN
    SELECT RAISE(ABORT, 'NOT NULL constraint failed: benchmark_runs.status');
END;

CREATE INDEX benchmark_runs_status_idx ON benchmark_runs (status);
