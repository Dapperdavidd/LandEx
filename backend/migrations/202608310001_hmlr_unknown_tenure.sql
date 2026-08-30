ALTER TABLE hmlr_price_paid_transactions
    DROP CONSTRAINT hmlr_tenure_valid;

ALTER TABLE hmlr_price_paid_transactions
    ADD CONSTRAINT hmlr_tenure_valid CHECK (tenure IN ('F', 'L', 'U'));
