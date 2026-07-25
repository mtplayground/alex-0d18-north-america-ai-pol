CREATE TABLE sources (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    region TEXT NOT NULL CHECK (region IN ('us', 'ca')),
    agency TEXT NOT NULL,
    base_url TEXT NOT NULL,
    crawl_config JSONB NOT NULL DEFAULT '{}'::jsonb,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT sources_region_agency_base_url_key UNIQUE (region, agency, base_url)
);

CREATE INDEX sources_enabled_region_idx ON sources (region) WHERE enabled;

INSERT INTO sources (region, agency, base_url, crawl_config)
VALUES
    (
        'us',
        'Office of the Federal Register',
        'https://www.federalregister.gov',
        '{
            "start_paths": ["/documents/search"],
            "allowed_path_prefixes": ["/documents/"],
            "poll_interval_minutes": 360
        }'::jsonb
    ),
    (
        'us',
        'National Institute of Standards and Technology',
        'https://www.nist.gov',
        '{
            "start_paths": ["/artificial-intelligence"],
            "allowed_path_prefixes": ["/artificial-intelligence/"],
            "poll_interval_minutes": 720
        }'::jsonb
    ),
    (
        'ca',
        'Canada Gazette',
        'https://gazette.gc.ca',
        '{
            "start_paths": ["/rp-pr/publications-eng.html"],
            "allowed_path_prefixes": ["/rp-pr/p1/", "/rp-pr/p2/"],
            "poll_interval_minutes": 720
        }'::jsonb
    ),
    (
        'ca',
        'Treasury Board of Canada Secretariat',
        'https://www.canada.ca/en/treasury-board-secretariat.html',
        '{
            "start_paths": ["/en/treasury-board-secretariat/services/information-technology.html"],
            "allowed_path_prefixes": ["/en/treasury-board-secretariat/services/"],
            "poll_interval_minutes": 720
        }'::jsonb
    )
ON CONFLICT ON CONSTRAINT sources_region_agency_base_url_key DO NOTHING;
