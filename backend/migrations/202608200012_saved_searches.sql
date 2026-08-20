CREATE TABLE saved_searches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    criteria JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT saved_searches_name_length CHECK (char_length(name) BETWEEN 1 AND 100),
    CONSTRAINT saved_searches_criteria_object CHECK (jsonb_typeof(criteria) = 'object')
);

CREATE UNIQUE INDEX saved_searches_user_name_idx
    ON saved_searches (user_id, LOWER(name));
CREATE INDEX saved_searches_user_updated_idx
    ON saved_searches (user_id, updated_at DESC);
