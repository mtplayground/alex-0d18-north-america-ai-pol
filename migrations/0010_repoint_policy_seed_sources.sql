-- Replace broad landing pages with source-owned item-level endpoints.  These
-- updates retain the original source rows so existing entry/audit references
-- remain valid while future ingestion uses structured documents.

-- The Federal Register API returns document result objects, including stable
-- document numbers and canonical HTML URLs, for the AI-related search term.
UPDATE sources
SET
    crawl_config = '{
        "start_paths": ["/api/v1/documents.json?conditions%5Bterm%5D=artificial%20intelligence&per_page=100&order=newest"],
        "format": "json",
        "poll_interval_minutes": 360
    }'::jsonb,
    updated_at = NOW()
WHERE agency = 'Office of the Federal Register'
  AND category = 'policy';

-- NIST publishes a topic-specific RSS feed whose items link to individual AI
-- news/articles.  Classify it as news so the feed normalizer is selected.
UPDATE sources
SET
    category = 'news',
    crawl_config = '{
        "start_paths": ["/news-events/artificial%20intelligence/rss.xml"],
        "feed_format": "rss",
        "poll_interval_minutes": 720
    }'::jsonb,
    updated_at = NOW()
WHERE agency = 'National Institute of Standards and Technology'
  AND category = 'policy';

-- Canada Gazette exposes item-level feeds for Parts I (notices/proposed
-- regulations) and II (official regulations), unlike its publications index.
UPDATE sources
SET
    crawl_config = '{
        "start_paths": ["/rss/p1-eng.xml", "/rss/p2-eng.xml"],
        "feed_format": "rss",
        "poll_interval_minutes": 720
    }'::jsonb,
    updated_at = NOW()
WHERE agency = 'Canada Gazette'
  AND category = 'policy';

-- The Treasury Board seed is a broad landing page and has no verified clean
-- item-level publication feed.  Disable it rather than continue ingesting
-- navigation content; retain its configuration for operator review.
UPDATE sources
SET
    enabled = FALSE,
    crawl_config = crawl_config || '{
        "disabled_reason": "No verified item-level publication feed is available for this seed source"
    }'::jsonb,
    updated_at = NOW()
WHERE agency = 'Treasury Board of Canada Secretariat'
  AND category = 'policy';
