-- Preserve a durable per-source audit trail for ingestion, including partial runs.
CREATE TABLE source_ingestion_runs (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    source_id BIGINT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
    raw_snapshot_key TEXT,
    records_processed INTEGER NOT NULL DEFAULT 0,
    new_entries INTEGER NOT NULL DEFAULT 0,
    updated_entries INTEGER NOT NULL DEFAULT 0,
    unchanged_entries INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX source_ingestion_runs_source_started_idx
    ON source_ingestion_runs (source_id, started_at DESC);
