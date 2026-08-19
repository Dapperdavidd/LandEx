ALTER TABLE listings DROP CONSTRAINT listings_status_valid;

ALTER TABLE listings ADD CONSTRAINT listings_status_valid CHECK (
    status IN (
        'active',
        'inactive',
        'pending',
        'sold',
        'rented',
        'withdrawn',
        'expired',
        'unknown'
    )
);
