CREATE TABLE alert_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    watchlist_item_id UUID NOT NULL REFERENCES watchlist_items(id) ON DELETE CASCADE,
    alert_type TEXT NOT NULL,
    threshold NUMERIC(20, 4),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT alert_rules_type_valid CHECK (alert_type IN ('price_change', 'rent_change', 'yield_change', 'listing_status_change', 'market_change', 'new_match')),
    CONSTRAINT alert_rules_threshold_valid CHECK (threshold IS NULL OR threshold >= 0),
    UNIQUE (user_id, watchlist_item_id, alert_type)
);

CREATE INDEX alert_rules_user_enabled_idx ON alert_rules(user_id, enabled, created_at DESC);

CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    alert_rule_id UUID REFERENCES alert_rules(id) ON DELETE SET NULL,
    notification_type TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::JSONB,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT notifications_type_valid CHECK (notification_type IN ('alert', 'system')),
    CONSTRAINT notifications_title_length CHECK (char_length(title) BETWEEN 1 AND 200),
    CONSTRAINT notifications_body_length CHECK (char_length(body) BETWEEN 1 AND 2000)
);

CREATE INDEX notifications_user_unread_idx ON notifications(user_id, read_at, created_at DESC);
