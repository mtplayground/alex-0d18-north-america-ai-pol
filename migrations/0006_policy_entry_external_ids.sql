ALTER TABLE policy_entries
    ADD COLUMN source_external_id TEXT;

UPDATE policy_entries
SET source_external_id = source_url
WHERE source_external_id IS NULL;

ALTER TABLE policy_entries
    ALTER COLUMN source_external_id SET NOT NULL,
    DROP CONSTRAINT policy_entries_source_source_url_key,
    ADD CONSTRAINT policy_entries_source_external_id_key UNIQUE (source_id, source_external_id);
