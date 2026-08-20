ALTER TABLE alert_rules
    DROP CONSTRAINT alert_rules_user_id_watchlist_item_id_alert_type_key,
    ALTER COLUMN watchlist_item_id DROP NOT NULL,
    ADD COLUMN saved_search_id UUID REFERENCES saved_searches(id) ON DELETE CASCADE;

-- Legacy new-match rules had no saved-search target and cannot be evaluated safely.
DELETE FROM alert_rules WHERE alert_type = 'new_match';

ALTER TABLE alert_rules ADD CONSTRAINT alert_rules_one_target
    CHECK (num_nonnulls(watchlist_item_id, saved_search_id) = 1);
ALTER TABLE alert_rules ADD CONSTRAINT alert_rules_target_type
    CHECK ((alert_type = 'new_match' AND saved_search_id IS NOT NULL)
        OR (alert_type <> 'new_match' AND watchlist_item_id IS NOT NULL));

CREATE UNIQUE INDEX alert_rules_watchlist_type_idx
    ON alert_rules (user_id, watchlist_item_id, alert_type) WHERE watchlist_item_id IS NOT NULL;
CREATE UNIQUE INDEX alert_rules_saved_search_type_idx
    ON alert_rules (user_id, saved_search_id, alert_type) WHERE saved_search_id IS NOT NULL;
