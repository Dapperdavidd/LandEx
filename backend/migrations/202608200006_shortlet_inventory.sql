ALTER TABLE listings DROP CONSTRAINT listings_type_valid;
ALTER TABLE listings ADD CONSTRAINT listings_type_valid
    CHECK (listing_type IN ('sale', 'rent', 'shortlet'));

ALTER TABLE listings ADD COLUMN price_period TEXT NOT NULL DEFAULT 'total';
UPDATE listings SET price_period = 'month' WHERE listing_type = 'rent';
ALTER TABLE listings ADD CONSTRAINT listings_price_period_valid
    CHECK (price_period IN ('total', 'month', 'night'));
ALTER TABLE listings ADD CONSTRAINT listings_type_period_valid CHECK (
    (listing_type = 'sale' AND price_period = 'total')
    OR (listing_type = 'rent' AND price_period = 'month')
    OR (listing_type = 'shortlet' AND price_period = 'night')
);

ALTER TABLE property_observations ADD COLUMN shortlet_price_nightly NUMERIC(20, 4);
ALTER TABLE property_observations ADD CONSTRAINT property_observations_shortlet_price_valid
    CHECK (shortlet_price_nightly IS NULL OR shortlet_price_nightly >= 0);

ALTER TABLE properties ALTER COLUMN latitude DROP NOT NULL;
ALTER TABLE properties ALTER COLUMN longitude DROP NOT NULL;
ALTER TABLE properties ADD CONSTRAINT properties_coordinate_pair CHECK (
    (latitude IS NULL AND longitude IS NULL) OR (latitude IS NOT NULL AND longitude IS NOT NULL)
);
