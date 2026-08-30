CREATE TABLE data_import_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_slug TEXT NOT NULL,
    dataset_key TEXT NOT NULL,
    source_url TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    phase TEXT NOT NULL DEFAULT 'download',
    checkpoint BIGINT NOT NULL DEFAULT 0,
    rows_processed BIGINT NOT NULL DEFAULT 0,
    bytes_downloaded BIGINT NOT NULL DEFAULT 0,
    checksum_sha256 TEXT,
    error_message TEXT,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT data_import_runs_status_valid CHECK (status IN ('pending', 'running', 'failed', 'completed')),
    CONSTRAINT data_import_runs_phase_valid CHECK (phase IN ('download', 'import', 'aggregate', 'complete')),
    CONSTRAINT data_import_runs_counters_valid CHECK (checkpoint >= 0 AND rows_processed >= 0 AND bytes_downloaded >= 0),
    UNIQUE (source_slug, dataset_key)
);

CREATE TABLE hmlr_price_paid_transactions (
    transaction_id UUID PRIMARY KEY,
    price NUMERIC(20, 2) NOT NULL,
    transferred_on DATE NOT NULL,
    postcode TEXT,
    property_type CHAR(1) NOT NULL,
    new_build BOOLEAN NOT NULL,
    tenure CHAR(1) NOT NULL,
    paon TEXT,
    saon TEXT,
    street TEXT,
    locality TEXT,
    town_city TEXT NOT NULL,
    district TEXT,
    county TEXT,
    ppd_category CHAR(1) NOT NULL,
    record_status CHAR(1) NOT NULL,
    source_dataset TEXT NOT NULL,
    imported_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT hmlr_price_positive CHECK (price > 0),
    CONSTRAINT hmlr_property_type_valid CHECK (property_type IN ('D', 'S', 'T', 'F', 'O')),
    CONSTRAINT hmlr_tenure_valid CHECK (tenure IN ('F', 'L')),
    CONSTRAINT hmlr_ppd_category_valid CHECK (ppd_category IN ('A', 'B')),
    CONSTRAINT hmlr_record_status_valid CHECK (record_status IN ('A', 'C', 'D'))
);

CREATE INDEX hmlr_price_paid_date_idx ON hmlr_price_paid_transactions(transferred_on DESC);
CREATE INDEX hmlr_price_paid_town_idx ON hmlr_price_paid_transactions(town_city, transferred_on DESC);
CREATE INDEX hmlr_price_paid_postcode_idx ON hmlr_price_paid_transactions(postcode, transferred_on DESC)
    WHERE postcode IS NOT NULL;

CREATE TABLE external_market_series (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_id UUID NOT NULL REFERENCES providers(id) ON DELETE RESTRICT,
    source_series_id TEXT NOT NULL,
    location_id UUID REFERENCES locations(id) ON DELETE SET NULL,
    geography_code TEXT,
    geography_name TEXT NOT NULL,
    country_code CHAR(2),
    metric TEXT NOT NULL,
    property_type TEXT,
    frequency TEXT NOT NULL,
    unit TEXT NOT NULL,
    currency CHAR(3),
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT external_market_series_country_format CHECK (country_code IS NULL OR country_code ~ '^[A-Z]{2}$'),
    CONSTRAINT external_market_series_currency_format CHECK (currency IS NULL OR currency ~ '^[A-Z]{3}$'),
    CONSTRAINT external_market_series_frequency_valid CHECK (frequency IN ('daily', 'monthly', 'quarterly', 'annual')),
    UNIQUE (provider_id, source_series_id)
);

CREATE TABLE external_market_observations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    series_id UUID NOT NULL REFERENCES external_market_series(id) ON DELETE CASCADE,
    observed_on DATE NOT NULL,
    value NUMERIC(24, 8) NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (series_id, observed_on)
);

CREATE INDEX external_market_observations_history_idx
    ON external_market_observations(series_id, observed_on DESC);
