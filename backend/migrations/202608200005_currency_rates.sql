CREATE TABLE currency_rates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider TEXT NOT NULL,
    base_currency CHAR(3) NOT NULL,
    quote_currency CHAR(3) NOT NULL,
    rate NUMERIC(28, 12) NOT NULL,
    observed_on DATE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT currency_rates_provider_length CHECK (char_length(provider) BETWEEN 1 AND 50),
    CONSTRAINT currency_rates_base_format CHECK (base_currency ~ '^[A-Z]{3}$'),
    CONSTRAINT currency_rates_quote_format CHECK (quote_currency ~ '^[A-Z]{3}$'),
    CONSTRAINT currency_rates_distinct_pair CHECK (base_currency <> quote_currency),
    CONSTRAINT currency_rates_positive CHECK (rate > 0),
    UNIQUE (provider, base_currency, quote_currency, observed_on)
);

CREATE INDEX currency_rates_pair_history_idx
    ON currency_rates(base_currency, quote_currency, observed_on DESC, created_at DESC);
