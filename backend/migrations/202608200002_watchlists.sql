CREATE TABLE watchlists (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT watchlists_name_length CHECK (char_length(name) BETWEEN 1 AND 100),
    UNIQUE (user_id, name)
);

CREATE INDEX watchlists_user_id_idx ON watchlists(user_id, created_at DESC);

CREATE TABLE watchlist_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    watchlist_id UUID NOT NULL REFERENCES watchlists(id) ON DELETE CASCADE,
    property_id UUID REFERENCES properties(id) ON DELETE CASCADE,
    market_id UUID REFERENCES markets(id) ON DELETE CASCADE,
    location_id UUID REFERENCES locations(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT watchlist_items_exactly_one_target CHECK (
        num_nonnulls(property_id, market_id, location_id) = 1
    ),
    UNIQUE NULLS NOT DISTINCT (watchlist_id, property_id, market_id, location_id)
);

CREATE INDEX watchlist_items_watchlist_id_idx
    ON watchlist_items(watchlist_id, created_at DESC);
