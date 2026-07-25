ALTER TABLE policy_versions
    ADD COLUMN raw_snapshot_key TEXT;

CREATE INDEX policy_versions_raw_snapshot_key_idx
    ON policy_versions (raw_snapshot_key)
    WHERE raw_snapshot_key IS NOT NULL;
