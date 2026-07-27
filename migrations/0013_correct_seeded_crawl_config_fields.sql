-- Audit of the sources added in 0012_expand_ai_policy_and_news_sources.sql:
-- * DeepMind, Stanford HAI, White House OSTP, Copyright Office, and Canada.ca
--   use XML sitemap endpoints in sitemap_paths.
-- * NVIDIA Deep Learning Blog and ISED Canada use RSS endpoints in feed_paths.
-- * AI.gov exposes /sitemap.xml, which is a sitemap rather than an RSS feed.
-- No other seeded source from that migration has a crawl-config field mismatch.

-- Retype the AI.gov discovery entry point without changing its explicit start
-- path, bounded-crawl settings, filters, or polling interval.
UPDATE sources
SET
    crawl_config = (crawl_config - 'feed_paths' - 'feed_format')
        || jsonb_build_object('sitemap_paths', jsonb_build_array('/sitemap.xml')),
    updated_at = NOW()
WHERE region = 'us'
  AND category = 'policy'
  AND agency = 'National Artificial Intelligence Initiative Office'
  AND base_url = 'https://www.ai.gov';
