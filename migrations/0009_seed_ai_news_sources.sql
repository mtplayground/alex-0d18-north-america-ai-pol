-- Starter feeds make the news category useful immediately after migration.
-- URLs and crawler settings remain data in sources.crawl_config so they can be
-- changed or disabled later without a code release.
INSERT INTO sources (region, category, agency, base_url, crawl_config)
VALUES
    (
        'global',
        'news',
        'OpenAI News',
        'https://openai.com',
        '{
            "start_paths": ["/news/rss.xml"],
            "feed_format": "rss",
            "poll_interval_minutes": 60
        }'::jsonb
    ),
    (
        'global',
        'news',
        'Google AI Blog',
        'https://blog.google',
        '{
            "start_paths": ["/technology/ai/rss/"],
            "feed_format": "rss",
            "poll_interval_minutes": 60
        }'::jsonb
    ),
    (
        'global',
        'news',
        'Hugging Face Blog',
        'https://huggingface.co',
        '{
            "start_paths": ["/blog/feed.xml"],
            "feed_format": "rss",
            "poll_interval_minutes": 120
        }'::jsonb
    ),
    (
        'global',
        'news',
        'MIT Technology Review AI',
        'https://www.technologyreview.com',
        '{
            "start_paths": ["/topic/artificial-intelligence/feed/"],
            "feed_format": "rss",
            "poll_interval_minutes": 120
        }'::jsonb
    )
ON CONFLICT ON CONSTRAINT sources_region_agency_base_url_key DO NOTHING;
