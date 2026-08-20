CREATE TABLE nearby_features (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source TEXT NOT NULL CHECK (source IN ('openstreetmap')),
    source_element_type TEXT NOT NULL CHECK (source_element_type IN ('node', 'way', 'relation')),
    source_id BIGINT NOT NULL,
    category TEXT NOT NULL CHECK (
        category IN ('transport', 'education', 'healthcare', 'commerce', 'leisure', 'infrastructure')
    ),
    kind TEXT NOT NULL CHECK (kind ~ '^[a-z0-9_:-]+$'),
    name TEXT,
    latitude DOUBLE PRECISION NOT NULL CHECK (latitude BETWEEN -90 AND 90),
    longitude DOUBLE PRECISION NOT NULL CHECK (longitude BETWEEN -180 AND 180),
    tags JSONB NOT NULL DEFAULT '{}'::JSONB,
    source_updated_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (source, source_element_type, source_id)
);

CREATE INDEX nearby_features_coordinates_idx ON nearby_features(latitude, longitude);
CREATE INDEX nearby_features_category_kind_idx ON nearby_features(category, kind);

CREATE TABLE property_nearby_features (
    property_id UUID NOT NULL REFERENCES properties(id) ON DELETE CASCADE,
    feature_id UUID NOT NULL REFERENCES nearby_features(id) ON DELETE CASCADE,
    distance_meters INTEGER NOT NULL CHECK (distance_meters >= 0),
    query_radius_meters INTEGER NOT NULL CHECK (query_radius_meters BETWEEN 100 AND 10000),
    observed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (property_id, feature_id),
    CHECK (distance_meters <= query_radius_meters),
    CHECK (expires_at > observed_at)
);

CREATE INDEX property_nearby_features_lookup_idx
    ON property_nearby_features(property_id, distance_meters, feature_id);
CREATE INDEX property_nearby_features_expiry_idx ON property_nearby_features(expires_at);
