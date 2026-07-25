CREATE TABLE policy_entries (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    source_id BIGINT NOT NULL REFERENCES sources (id) ON DELETE RESTRICT,
    title TEXT NOT NULL,
    region TEXT NOT NULL CHECK (region IN ('us', 'ca')),
    agency TEXT NOT NULL,
    publication_date DATE,
    status TEXT NOT NULL,
    source_url TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT policy_entries_source_source_url_key UNIQUE (source_id, source_url)
);

CREATE INDEX policy_entries_publication_date_idx
    ON policy_entries (publication_date DESC NULLS LAST, id DESC);

CREATE INDEX policy_entries_region_agency_status_date_idx
    ON policy_entries (region, agency, status, publication_date DESC NULLS LAST, id DESC);
