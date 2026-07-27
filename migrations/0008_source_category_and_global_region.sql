-- Sources remain policy sources by default so the existing seeded configuration
-- and all previously persisted data retain their current semantics.
ALTER TABLE sources
    ADD COLUMN category TEXT NOT NULL DEFAULT 'policy',
    ADD CONSTRAINT sources_category_check CHECK (category IN ('policy', 'news'));

-- Policy sources remain limited to their established North American regions.
-- News sources may be global while still allowing regional news sources later.
ALTER TABLE sources
    DROP CONSTRAINT sources_region_check,
    ADD CONSTRAINT sources_region_category_check CHECK (
        (category = 'policy' AND region IN ('us', 'ca'))
        OR (category = 'news' AND region IN ('us', 'ca', 'global'))
    );

-- Entries inherit their category from sources at query time. Their region must
-- nevertheless accept global records once a news normalizer is introduced.
ALTER TABLE policy_entries
    DROP CONSTRAINT policy_entries_region_check,
    ADD CONSTRAINT policy_entries_region_check CHECK (region IN ('us', 'ca', 'global'));
