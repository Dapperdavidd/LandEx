CREATE TABLE paper_account_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES paper_accounts(id) ON DELETE CASCADE,
    observed_on DATE NOT NULL DEFAULT CURRENT_DATE,
    base_currency VARCHAR(3) NOT NULL,
    cash_balance NUMERIC(20, 4) NOT NULL,
    positions_value NUMERIC(24, 8) NOT NULL,
    total_value NUMERIC(24, 8) NOT NULL,
    net_funding NUMERIC(24, 8) NOT NULL,
    total_pnl NUMERIC(24, 8) NOT NULL,
    total_return_percent NUMERIC(16, 4) NOT NULL,
    realized_pnl NUMERIC(24, 8) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (account_id, observed_on)
);

CREATE INDEX paper_account_snapshots_account_history_idx
    ON paper_account_snapshots (account_id, observed_on DESC);
