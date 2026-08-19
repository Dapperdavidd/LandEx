CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    display_name TEXT NOT NULL,
    primary_email TEXT NOT NULL,
    primary_email_normalized TEXT NOT NULL UNIQUE,
    email_verified_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT users_display_name_length CHECK (char_length(display_name) BETWEEN 1 AND 100),
    CONSTRAINT users_email_length CHECK (char_length(primary_email) BETWEEN 3 AND 320),
    CONSTRAINT users_status_valid CHECK (status IN ('active', 'suspended', 'deleted'))
);

CREATE TABLE user_identities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    provider_subject TEXT NOT NULL,
    email TEXT,
    email_normalized TEXT,
    password_hash TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT user_identities_provider_valid CHECK (provider IN ('email', 'google', 'apple')),
    CONSTRAINT user_identities_password_shape CHECK (
        (provider = 'email' AND password_hash IS NOT NULL AND email_normalized IS NOT NULL)
        OR (provider <> 'email' AND password_hash IS NULL)
    ),
    UNIQUE (provider, provider_subject),
    UNIQUE (user_id, provider)
);

CREATE INDEX user_identities_user_id_idx ON user_identities(user_id);
CREATE UNIQUE INDEX user_identities_email_login_idx
    ON user_identities(email_normalized)
    WHERE provider = 'email';

CREATE TABLE user_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    access_token_hash BYTEA NOT NULL UNIQUE,
    refresh_token_hash BYTEA NOT NULL UNIQUE,
    access_expires_at TIMESTAMPTZ NOT NULL,
    refresh_expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT user_sessions_expiry_order CHECK (refresh_expires_at > access_expires_at)
);

CREATE INDEX user_sessions_active_user_idx ON user_sessions(user_id, refresh_expires_at)
    WHERE revoked_at IS NULL;
