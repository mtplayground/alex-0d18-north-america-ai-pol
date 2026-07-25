CREATE TYPE policy_change_kind AS ENUM ('new', 'updated');

CREATE TABLE policy_versions (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    policy_entry_id BIGINT NOT NULL REFERENCES policy_entries (id) ON DELETE CASCADE,
    version_number INTEGER NOT NULL CHECK (version_number > 0),
    change_kind policy_change_kind NOT NULL,
    canonical_content JSONB NOT NULL,
    content_hash TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    change_summary TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT policy_versions_entry_version_number_key UNIQUE (policy_entry_id, version_number)
);

CREATE INDEX policy_versions_entry_observed_at_idx
    ON policy_versions (policy_entry_id, observed_at DESC, id DESC);

CREATE INDEX policy_versions_observed_at_idx
    ON policy_versions (observed_at DESC, id DESC);
