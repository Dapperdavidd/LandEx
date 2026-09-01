CREATE TABLE investment_instruments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    instrument_kind TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'research',
    country_code CHAR(2) NOT NULL,
    currency CHAR(3) NOT NULL,
    symbol TEXT,
    exchange TEXT,
    location_id UUID REFERENCES locations(id) ON DELETE SET NULL,
    property_id UUID REFERENCES properties(id) ON DELETE SET NULL,
    provider_id UUID REFERENCES providers(id) ON DELETE SET NULL,
    source_url TEXT,
    valuation_method TEXT NOT NULL,
    liquidity_class TEXT NOT NULL DEFAULT 'unknown',
    real_money_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT investment_instruments_kind_valid CHECK (
        instrument_kind IN ('direct_property', 'listed_security', 'fractional_offering', 'market_proxy')
    ),
    CONSTRAINT investment_instruments_status_valid CHECK (
        status IN ('research', 'paper_tradeable', 'real_investible', 'inactive')
    ),
    CONSTRAINT investment_instruments_liquidity_valid CHECK (
        liquidity_class IN ('listed', 'index_proxy', 'illiquid', 'unknown')
    ),
    CONSTRAINT investment_instruments_country_format CHECK (country_code ~ '^[A-Z]{2}$'),
    CONSTRAINT investment_instruments_currency_format CHECK (currency ~ '^[A-Z]{3}$'),
    CONSTRAINT investment_instruments_real_money_guard CHECK (
        NOT real_money_enabled OR status = 'real_investible'
    )
);

CREATE INDEX investment_instruments_discovery_idx
    ON investment_instruments(country_code, instrument_kind, status, name);

CREATE TABLE instrument_observations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instrument_id UUID NOT NULL REFERENCES investment_instruments(id) ON DELETE CASCADE,
    observed_on DATE NOT NULL,
    value NUMERIC(24, 8) NOT NULL,
    currency CHAR(3) NOT NULL,
    annual_change_percent NUMERIC(12, 6),
    income_yield_percent NUMERIC(12, 6),
    source_url TEXT,
    methodology TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT instrument_observations_value_positive CHECK (value > 0),
    CONSTRAINT instrument_observations_currency_format CHECK (currency ~ '^[A-Z]{3}$'),
    UNIQUE (instrument_id, observed_on)
);

CREATE INDEX instrument_observations_history_idx
    ON instrument_observations(instrument_id, observed_on DESC);

CREATE TABLE country_coverage (
    country_code CHAR(2) PRIMARY KEY,
    country_name TEXT NOT NULL,
    coverage_depth TEXT NOT NULL DEFAULT 'planned',
    has_market_data BOOLEAN NOT NULL DEFAULT FALSE,
    has_historical_data BOOLEAN NOT NULL DEFAULT FALSE,
    has_active_listings BOOLEAN NOT NULL DEFAULT FALSE,
    has_investible_offerings BOOLEAN NOT NULL DEFAULT FALSE,
    provider_slugs JSONB NOT NULL DEFAULT '[]'::JSONB,
    methodology TEXT,
    latest_observation_on DATE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT country_coverage_depth_valid CHECK (
        coverage_depth IN ('planned', 'basic', 'standard', 'deep')
    ),
    CONSTRAINT country_coverage_provider_slugs_array CHECK (jsonb_typeof(provider_slugs) = 'array')
);
