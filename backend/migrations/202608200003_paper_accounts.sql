CREATE TABLE paper_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    base_currency CHAR(3) NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT paper_accounts_name_length CHECK (char_length(name) BETWEEN 1 AND 100),
    CONSTRAINT paper_accounts_currency_format CHECK (base_currency ~ '^[A-Z]{3}$'),
    CONSTRAINT paper_accounts_status_valid CHECK (status IN ('active', 'archived')),
    UNIQUE (user_id, name)
);

CREATE INDEX paper_accounts_user_id_idx ON paper_accounts(user_id, created_at DESC);

CREATE TABLE paper_cash_ledger (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES paper_accounts(id) ON DELETE CASCADE,
    entry_type TEXT NOT NULL,
    amount NUMERIC(20, 4) NOT NULL,
    description TEXT NOT NULL,
    reference_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT paper_cash_ledger_type_valid CHECK (
        entry_type IN ('initial_funding', 'purchase', 'sale', 'rental_income', 'expense', 'adjustment')
    ),
    CONSTRAINT paper_cash_ledger_amount_nonzero CHECK (amount <> 0),
    CONSTRAINT paper_cash_ledger_description_length CHECK (char_length(description) BETWEEN 1 AND 200)
);

CREATE INDEX paper_cash_ledger_account_idx
    ON paper_cash_ledger(account_id, created_at, id);

CREATE UNIQUE INDEX paper_cash_ledger_reference_idx
    ON paper_cash_ledger(account_id, entry_type, reference_id)
    WHERE reference_id IS NOT NULL;
