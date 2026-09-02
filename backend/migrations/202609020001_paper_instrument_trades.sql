CREATE TABLE paper_instrument_trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES paper_accounts(id) ON DELETE CASCADE,
    instrument_id UUID NOT NULL REFERENCES investment_instruments(id) ON DELETE RESTRICT,
    side TEXT NOT NULL,
    units NUMERIC(28, 12) NOT NULL,
    execution_price NUMERIC(24, 8) NOT NULL,
    gross_amount NUMERIC(20, 4) NOT NULL,
    settlement_currency CHAR(3) NOT NULL,
    observed_on DATE NOT NULL,
    executed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT paper_instrument_trades_side_valid CHECK (side IN ('buy', 'sell')),
    CONSTRAINT paper_instrument_trades_units_positive CHECK (units > 0),
    CONSTRAINT paper_instrument_trades_price_positive CHECK (execution_price > 0),
    CONSTRAINT paper_instrument_trades_amount_positive CHECK (gross_amount > 0),
    CONSTRAINT paper_instrument_trades_currency_format CHECK (settlement_currency ~ '^[A-Z]{3}$')
);

CREATE INDEX paper_instrument_trades_account_history_idx
    ON paper_instrument_trades(account_id, executed_at DESC, id DESC);
CREATE INDEX paper_instrument_trades_position_idx
    ON paper_instrument_trades(account_id, instrument_id, executed_at, id);
