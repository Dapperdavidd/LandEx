ALTER TABLE watchlist_items
    ADD COLUMN instrument_id UUID REFERENCES investment_instruments(id) ON DELETE CASCADE;

ALTER TABLE watchlist_items
    DROP CONSTRAINT watchlist_items_exactly_one_target,
    DROP CONSTRAINT watchlist_items_watchlist_id_property_id_market_id_location_key;

ALTER TABLE watchlist_items
    ADD CONSTRAINT watchlist_items_exactly_one_target CHECK (
        num_nonnulls(property_id, market_id, location_id, instrument_id) = 1
    ),
    ADD CONSTRAINT watchlist_items_unique_target
        UNIQUE NULLS NOT DISTINCT (watchlist_id, property_id, market_id, location_id, instrument_id);

CREATE INDEX watchlist_items_instrument_id_idx
    ON watchlist_items(instrument_id)
    WHERE instrument_id IS NOT NULL;
