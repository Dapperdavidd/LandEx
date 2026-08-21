-- Supports the latest-observation LATERAL lookups used by normalized Explore.
-- Partial indexes keep inactive/unusable rows out of the hot path.
CREATE INDEX listings_active_sale_property_recent_idx
    ON listings(property_id, last_seen_at DESC)
    WHERE status = 'active' AND listing_type = 'sale' AND price > 0;

CREATE INDEX property_observations_rent_recent_idx
    ON property_observations(property_id, observed_on DESC, created_at DESC)
    WHERE rental_price_monthly IS NOT NULL;

CREATE INDEX market_observations_growth_recent_idx
    ON market_observations(market_id, observed_on DESC, created_at DESC)
    WHERE annual_growth_percent IS NOT NULL;

CREATE INDEX score_observations_property_score_recent_idx
    ON score_observations(property_id, observed_on DESC, created_at DESC)
    WHERE overall_score IS NOT NULL;
