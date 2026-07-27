-- Remove source landing-page/navigation records that were persisted before
-- record-quality validation existed.  The predicate is intentionally limited
-- to the affected seeded government sources and the shared generic titles.

-- A source-run row has only one raw snapshot key.  Remove its audit reference
-- only when that snapshot is used exclusively by junk entries, so a mixed run
-- keeps its audit history for legitimate records.
WITH junk_entries AS (
    SELECT entries.id, entries.source_id
    FROM policy_entries AS entries
    INNER JOIN sources AS source ON source.id = entries.source_id
    WHERE source.agency IN (
        'Canada Gazette',
        'National Institute of Standards and Technology',
        'Office of the Federal Register'
    )
      AND lower(regexp_replace(btrim(entries.title), '\s+', ' ', 'g')) IN (
          'language selection',
          'request access',
          'artificial intelligence',
          'news and updates',
          'search'
      )
), junk_snapshot_runs AS (
    SELECT DISTINCT runs.id
    FROM source_ingestion_runs AS runs
    INNER JOIN policy_versions AS junk_version
        ON junk_version.raw_snapshot_key = runs.raw_snapshot_key
    INNER JOIN junk_entries
        ON junk_entries.id = junk_version.policy_entry_id
       AND junk_entries.source_id = runs.source_id
    WHERE runs.raw_snapshot_key IS NOT NULL
      AND NOT EXISTS (
          SELECT 1
          FROM policy_versions AS retained_version
          INNER JOIN policy_entries AS retained_entry
              ON retained_entry.id = retained_version.policy_entry_id
          WHERE retained_entry.source_id = runs.source_id
            AND retained_version.raw_snapshot_key = runs.raw_snapshot_key
            AND NOT EXISTS (
                SELECT 1
                FROM junk_entries
                WHERE junk_entries.id = retained_entry.id
            )
      )
)
DELETE FROM source_ingestion_runs AS runs
USING junk_snapshot_runs
WHERE runs.id = junk_snapshot_runs.id;

-- policy_versions reference policy_entries with ON DELETE CASCADE, which also
-- removes each junk entry's version history and raw snapshot references.
DELETE FROM policy_entries AS entries
USING sources AS source
WHERE source.id = entries.source_id
  AND source.agency IN (
      'Canada Gazette',
      'National Institute of Standards and Technology',
      'Office of the Federal Register'
  )
  AND lower(regexp_replace(btrim(entries.title), '\s+', ' ', 'g')) IN (
      'language selection',
      'request access',
      'artificial intelligence',
      'news and updates',
      'search'
  );
