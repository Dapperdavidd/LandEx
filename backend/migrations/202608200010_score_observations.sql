CREATE TABLE score_observations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    property_id UUID REFERENCES properties(id) ON DELETE CASCADE,
    market_id UUID REFERENCES markets(id) ON DELETE CASCADE,
    methodology_version TEXT NOT NULL,
    observed_on DATE NOT NULL,
    overall_score NUMERIC(7, 2),
    components JSONB NOT NULL,
    unavailable_components JSONB NOT NULL DEFAULT '[]'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT score_observations_one_subject CHECK (num_nonnulls(property_id, market_id)=1),
    CONSTRAINT score_observations_score_valid CHECK (overall_score IS NULL OR overall_score BETWEEN 0 AND 100),
    UNIQUE NULLS NOT DISTINCT (property_id, market_id, methodology_version, observed_on)
);

CREATE INDEX score_observations_property_history_idx ON score_observations(property_id, observed_on DESC);
CREATE INDEX score_observations_market_history_idx ON score_observations(market_id, observed_on DESC);
