CREATE TABLE provider_request_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_id UUID NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    cursor TEXT,
    outcome TEXT NOT NULL DEFAULT 'reserved',
    CONSTRAINT provider_request_attempts_outcome_valid CHECK (
        outcome IN ('reserved', 'succeeded', 'failed')
    )
);

CREATE INDEX provider_request_attempts_window_idx
    ON provider_request_attempts(provider_id, requested_at DESC);
