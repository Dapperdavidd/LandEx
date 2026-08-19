CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT providers_slug_format CHECK (slug ~ '^[a-z0-9]+(?:-[a-z0-9]+)*$')
);

CREATE TABLE locations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_id UUID REFERENCES locations(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    country_code CHAR(2) NOT NULL,
    administrative_code TEXT,
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,
    population BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT locations_kind_valid CHECK (
        kind IN ('country', 'region', 'city', 'district', 'neighborhood')
    ),
    CONSTRAINT locations_country_code_format CHECK (country_code ~ '^[A-Z]{2}$'),
    CONSTRAINT locations_latitude_valid CHECK (latitude IS NULL OR latitude BETWEEN -90 AND 90),
    CONSTRAINT locations_longitude_valid CHECK (longitude IS NULL OR longitude BETWEEN -180 AND 180),
    CONSTRAINT locations_population_valid CHECK (population IS NULL OR population >= 0),
    UNIQUE NULLS NOT DISTINCT (parent_id, kind, normalized_name, country_code)
);

CREATE INDEX locations_parent_id_idx ON locations(parent_id);
CREATE INDEX locations_country_kind_idx ON locations(country_code, kind);
CREATE INDEX locations_normalized_name_idx ON locations(normalized_name);

CREATE TABLE provider_locations (
    provider_id UUID NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    location_id UUID NOT NULL REFERENCES locations(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL,
    raw_payload JSONB NOT NULL DEFAULT '{}'::JSONB,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (provider_id, source_id),
    UNIQUE (provider_id, location_id)
);

CREATE TABLE properties (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    location_id UUID NOT NULL REFERENCES locations(id) ON DELETE RESTRICT,
    property_type TEXT NOT NULL,
    address_line TEXT,
    postal_code TEXT,
    latitude DOUBLE PRECISION NOT NULL,
    longitude DOUBLE PRECISION NOT NULL,
    bedrooms NUMERIC(5, 2),
    bathrooms NUMERIC(5, 2),
    area_sqm NUMERIC(14, 2),
    lot_size_sqm NUMERIC(14, 2),
    year_built SMALLINT,
    attributes JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT properties_type_valid CHECK (
        property_type IN ('apartment', 'house', 'commercial', 'land', 'hotel', 'retail', 'industrial', 'other')
    ),
    CONSTRAINT properties_latitude_valid CHECK (latitude BETWEEN -90 AND 90),
    CONSTRAINT properties_longitude_valid CHECK (longitude BETWEEN -180 AND 180),
    CONSTRAINT properties_bedrooms_valid CHECK (bedrooms IS NULL OR bedrooms >= 0),
    CONSTRAINT properties_bathrooms_valid CHECK (bathrooms IS NULL OR bathrooms >= 0),
    CONSTRAINT properties_area_valid CHECK (area_sqm IS NULL OR area_sqm > 0),
    CONSTRAINT properties_lot_size_valid CHECK (lot_size_sqm IS NULL OR lot_size_sqm > 0),
    CONSTRAINT properties_year_built_valid CHECK (
        year_built IS NULL OR year_built BETWEEN 1000 AND 2200
    )
);

CREATE INDEX properties_location_id_idx ON properties(location_id);
CREATE INDEX properties_type_idx ON properties(property_type);
CREATE INDEX properties_coordinates_idx ON properties(latitude, longitude);

CREATE TABLE provider_properties (
    provider_id UUID NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    property_id UUID NOT NULL REFERENCES properties(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL,
    raw_payload JSONB NOT NULL DEFAULT '{}'::JSONB,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (provider_id, source_id),
    UNIQUE (provider_id, property_id)
);

CREATE TABLE listings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    property_id UUID NOT NULL REFERENCES properties(id) ON DELETE CASCADE,
    provider_id UUID NOT NULL REFERENCES providers(id) ON DELETE RESTRICT,
    source_id TEXT NOT NULL,
    listing_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'unknown',
    price NUMERIC(20, 4) NOT NULL,
    currency CHAR(3) NOT NULL,
    listed_at TIMESTAMPTZ,
    removed_at TIMESTAMPTZ,
    source_url TEXT,
    raw_payload JSONB NOT NULL DEFAULT '{}'::JSONB,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT listings_type_valid CHECK (listing_type IN ('sale', 'rent')),
    CONSTRAINT listings_status_valid CHECK (
        status IN ('active', 'pending', 'sold', 'rented', 'withdrawn', 'expired', 'unknown')
    ),
    CONSTRAINT listings_price_valid CHECK (price >= 0),
    CONSTRAINT listings_currency_format CHECK (currency ~ '^[A-Z]{3}$'),
    CONSTRAINT listings_dates_valid CHECK (removed_at IS NULL OR removed_at >= first_seen_at),
    UNIQUE (provider_id, source_id)
);

CREATE INDEX listings_property_id_idx ON listings(property_id);
CREATE INDEX listings_active_search_idx ON listings(listing_type, price, currency)
    WHERE status = 'active';

CREATE TABLE property_observations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    property_id UUID NOT NULL REFERENCES properties(id) ON DELETE CASCADE,
    provider_id UUID REFERENCES providers(id) ON DELETE SET NULL,
    observed_on DATE NOT NULL,
    asking_price NUMERIC(20, 4),
    rental_price_monthly NUMERIC(20, 4),
    estimated_value NUMERIC(20, 4),
    currency CHAR(3) NOT NULL,
    days_on_market INTEGER,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT property_observations_amounts_valid CHECK (
        (asking_price IS NULL OR asking_price >= 0)
        AND (rental_price_monthly IS NULL OR rental_price_monthly >= 0)
        AND (estimated_value IS NULL OR estimated_value >= 0)
    ),
    CONSTRAINT property_observations_currency_format CHECK (currency ~ '^[A-Z]{3}$'),
    CONSTRAINT property_observations_days_valid CHECK (days_on_market IS NULL OR days_on_market >= 0),
    UNIQUE NULLS NOT DISTINCT (property_id, provider_id, observed_on)
);

CREATE INDEX property_observations_history_idx
    ON property_observations(property_id, observed_on DESC);

CREATE TABLE markets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    location_id UUID NOT NULL REFERENCES locations(id) ON DELETE CASCADE,
    property_type TEXT,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT markets_property_type_valid CHECK (
        property_type IS NULL OR property_type IN (
            'apartment', 'house', 'commercial', 'land', 'hotel', 'retail', 'industrial', 'other'
        )
    ),
    UNIQUE NULLS NOT DISTINCT (location_id, property_type)
);

CREATE TABLE market_observations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_id UUID NOT NULL REFERENCES markets(id) ON DELETE CASCADE,
    provider_id UUID REFERENCES providers(id) ON DELETE SET NULL,
    observed_on DATE NOT NULL,
    currency CHAR(3) NOT NULL,
    median_sale_price NUMERIC(20, 4),
    median_rent_monthly NUMERIC(20, 4),
    gross_yield_percent NUMERIC(9, 4),
    annual_growth_percent NUMERIC(9, 4),
    active_inventory INTEGER,
    days_on_market NUMERIC(10, 2),
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT market_observations_currency_format CHECK (currency ~ '^[A-Z]{3}$'),
    CONSTRAINT market_observations_amounts_valid CHECK (
        (median_sale_price IS NULL OR median_sale_price >= 0)
        AND (median_rent_monthly IS NULL OR median_rent_monthly >= 0)
    ),
    CONSTRAINT market_observations_inventory_valid CHECK (
        active_inventory IS NULL OR active_inventory >= 0
    ),
    CONSTRAINT market_observations_days_valid CHECK (days_on_market IS NULL OR days_on_market >= 0),
    UNIQUE NULLS NOT DISTINCT (market_id, provider_id, observed_on)
);

CREATE INDEX market_observations_history_idx
    ON market_observations(market_id, observed_on DESC);
