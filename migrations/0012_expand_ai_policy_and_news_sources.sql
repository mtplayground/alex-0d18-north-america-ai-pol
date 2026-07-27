-- Broaden source coverage without modifying or replacing existing configured
-- sources. Every row uses bounded discovery so scheduled ingestion remains
-- conservative even when a provider's sitemap or feed grows over time.
INSERT INTO sources (region, category, agency, base_url, crawl_config)
VALUES
    (
        'global',
        'news',
        'Google DeepMind Blog',
        'https://deepmind.google',
        '{
            "start_paths": ["/sitemap.xml"],
            "sitemap_paths": ["/sitemap.xml"],
            "allowed_path_prefixes": ["/discover/blog/"],
            "max_pages": 16,
            "max_depth": 2,
            "include_patterns": ["/discover/blog/"],
            "exclude_patterns": ["/(?:tag|category|author)/", "/discover/blog/$"],
            "poll_interval_minutes": 180
        }'::jsonb
    ),
    (
        'global',
        'news',
        'Stanford Institute for Human-Centered Artificial Intelligence',
        'https://hai.stanford.edu',
        '{
            "start_paths": ["/sitemap.xml"],
            "sitemap_paths": ["/sitemap.xml"],
            "allowed_path_prefixes": ["/news/"],
            "max_pages": 16,
            "max_depth": 2,
            "include_patterns": ["/news/"],
            "exclude_patterns": ["/(?:events|people|research)/", "/news/$"],
            "poll_interval_minutes": 360
        }'::jsonb
    ),
    (
        'global',
        'news',
        'NVIDIA Deep Learning Blog',
        'https://blogs.nvidia.com',
        '{
            "start_paths": ["/blog/category/deep-learning/feed/"],
            "feed_paths": ["/blog/category/deep-learning/feed/"],
            "allowed_path_prefixes": ["/blog/"],
            "max_pages": 12,
            "max_depth": 1,
            "include_patterns": ["/blog/"],
            "exclude_patterns": ["/(?:tag|category|author)/"],
            "feed_format": "rss",
            "poll_interval_minutes": 180
        }'::jsonb
    ),
    (
        'us',
        'policy',
        'National Artificial Intelligence Initiative Office',
        'https://www.ai.gov',
        '{
            "start_paths": ["/sitemap.xml"],
            "feed_paths": ["/sitemap.xml"],
            "allowed_path_prefixes": ["/"],
            "max_pages": 12,
            "max_depth": 1,
            "include_patterns": ["/(?:ai|artificial-intelligence|documents|news)/"],
            "exclude_patterns": ["/(?:search|tag|category)/"],
            "feed_format": "rss",
            "poll_interval_minutes": 360
        }'::jsonb
    ),
    (
        'us',
        'policy',
        'White House Office of Science and Technology Policy',
        'https://www.whitehouse.gov',
        '{
            "start_paths": ["/wp-sitemap.xml"],
            "sitemap_paths": ["/wp-sitemap.xml"],
            "allowed_path_prefixes": ["/ostp/"],
            "max_pages": 12,
            "max_depth": 2,
            "include_patterns": ["/(?:artificial-intelligence|ai)/"],
            "exclude_patterns": ["/(?:tag|category|search)/"],
            "poll_interval_minutes": 360
        }'::jsonb
    ),
    (
        'us',
        'policy',
        'United States Copyright Office Artificial Intelligence Initiative',
        'https://www.copyright.gov',
        '{
            "start_paths": ["/sitemap.xml"],
            "sitemap_paths": ["/sitemap.xml"],
            "allowed_path_prefixes": ["/ai/"],
            "max_pages": 12,
            "max_depth": 2,
            "include_patterns": ["/ai/"],
            "exclude_patterns": ["/(?:search|contact)/"],
            "poll_interval_minutes": 720
        }'::jsonb
    ),
    (
        'ca',
        'policy',
        'Treasury Board of Canada Secretariat Digital Government',
        'https://www.canada.ca',
        '{
            "start_paths": ["/sitemap.xml"],
            "sitemap_paths": ["/sitemap.xml"],
            "allowed_path_prefixes": ["/en/government/system/digital-government/digital-government-innovations/responsible-use-ai/"],
            "max_pages": 12,
            "max_depth": 2,
            "include_patterns": ["/responsible-use-ai/"],
            "exclude_patterns": ["/(?:search|contact|services)/"],
            "poll_interval_minutes": 720
        }'::jsonb
    ),
    (
        'ca',
        'policy',
        'Innovation, Science and Economic Development Canada',
        'https://ised-isde.canada.ca',
        '{
            "start_paths": ["/site/ised/en/rss.xml"],
            "feed_paths": ["/site/ised/en/rss.xml"],
            "allowed_path_prefixes": ["/site/ised/en/"],
            "max_pages": 12,
            "max_depth": 1,
            "include_patterns": ["/(?:artificial-intelligence|ai)/"],
            "exclude_patterns": ["/(?:search|tag|category)/"],
            "feed_format": "rss",
            "poll_interval_minutes": 360
        }'::jsonb
    )
ON CONFLICT ON CONSTRAINT sources_region_agency_base_url_key DO NOTHING;
