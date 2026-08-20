ALTER TABLE alert_rules
    ADD COLUMN last_numeric_value NUMERIC(20, 6),
    ADD COLUMN last_text_value TEXT,
    ADD COLUMN last_evaluated_at TIMESTAMPTZ;

ALTER TABLE notifications ADD COLUMN deduplication_key TEXT;
CREATE UNIQUE INDEX notifications_user_deduplication_idx
    ON notifications(user_id, deduplication_key)
    WHERE deduplication_key IS NOT NULL;
