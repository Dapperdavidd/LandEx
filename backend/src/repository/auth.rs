use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::{ACCESS_TOKEN_MINUTES, REFRESH_TOKEN_DAYS, generate_token, token_hash};

#[derive(Clone)]
pub struct AuthRepository {
    pool: PgPool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserRecord {
    pub id: Uuid,
    pub display_name: String,
    pub primary_email: String,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct PasswordIdentity {
    pub user_id: Uuid,
    pub password_hash: String,
}

#[derive(Debug, Serialize)]
pub struct SessionTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
}

impl AuthRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_email_user(
        &self,
        display_name: &str,
        email: &str,
        normalized_email: &str,
        password_hash: &str,
    ) -> Result<UserRecord, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let user = sqlx::query_as::<_, UserRecord>(
            r#"
            INSERT INTO users (display_name, primary_email, primary_email_normalized)
            VALUES ($1, $2, $3)
            RETURNING id, display_name, primary_email, email_verified_at, created_at
            "#,
        )
        .bind(display_name)
        .bind(email)
        .bind(normalized_email)
        .fetch_one(&mut *transaction)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO user_identities (
                user_id, provider, provider_subject, email, email_normalized, password_hash
            ) VALUES ($1, 'email', $2, $3, $2, $4)
            "#,
        )
        .bind(user.id)
        .bind(normalized_email)
        .bind(email)
        .bind(password_hash)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(user)
    }

    pub async fn find_password_identity(
        &self,
        normalized_email: &str,
    ) -> Result<Option<PasswordIdentity>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT user_id, password_hash
            FROM user_identities
            WHERE provider = 'email' AND email_normalized = $1
            "#,
        )
        .bind(normalized_email)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn find_active_user(&self, user_id: Uuid) -> Result<Option<UserRecord>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT id, display_name, primary_email, email_verified_at, created_at
            FROM users
            WHERE id = $1 AND status = 'active'
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn create_session(&self, user_id: Uuid) -> Result<SessionTokens, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let tokens = insert_session(&mut transaction, user_id).await?;
        transaction.commit().await?;
        Ok(tokens)
    }

    pub async fn find_or_create_google_user(
        &self,
        subject: &str,
        email: &str,
        normalized_email: &str,
        display_name: &str,
    ) -> Result<UserRecord, GoogleIdentityError> {
        let mut transaction = self.pool.begin().await?;
        if let Some(user) = find_external_user(&mut transaction, "google", subject).await? {
            transaction.commit().await?;
            return Ok(user);
        }

        let email_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM users WHERE primary_email_normalized = $1)",
        )
        .bind(normalized_email)
        .fetch_one(&mut *transaction)
        .await?;
        if email_exists {
            transaction.rollback().await?;
            return Err(GoogleIdentityError::LinkRequired);
        }

        let user = sqlx::query_as::<_, UserRecord>(
            r#"
            INSERT INTO users (
                display_name, primary_email, primary_email_normalized, email_verified_at
            ) VALUES ($1, $2, $3, NOW())
            RETURNING id, display_name, primary_email, email_verified_at, created_at
            "#,
        )
        .bind(display_name)
        .bind(email)
        .bind(normalized_email)
        .fetch_one(&mut *transaction)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO user_identities (
                user_id, provider, provider_subject, email, email_normalized
            ) VALUES ($1, 'google', $2, $3, $4)
            "#,
        )
        .bind(user.id)
        .bind(subject)
        .bind(email)
        .bind(normalized_email)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(user)
    }

    pub async fn link_google_identity(
        &self,
        user_id: Uuid,
        subject: &str,
        email: &str,
        normalized_email: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO user_identities (
                user_id, provider, provider_subject, email, email_normalized
            ) VALUES ($1, 'google', $2, $3, $4)
            "#,
        )
        .bind(user_id)
        .bind(subject)
        .bind(email)
        .bind(normalized_email)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn authenticate_access_token(
        &self,
        access_token: &str,
    ) -> Result<Option<UserRecord>, sqlx::Error> {
        sqlx::query_as(
            r#"
            UPDATE user_sessions AS session
            SET last_used_at = NOW()
            FROM users
            WHERE session.access_token_hash = $1
              AND session.revoked_at IS NULL
              AND session.access_expires_at > NOW()
              AND users.id = session.user_id
              AND users.status = 'active'
            RETURNING users.id, users.display_name, users.primary_email,
                      users.email_verified_at, users.created_at
            "#,
        )
        .bind(token_hash(access_token))
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn rotate_refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<Option<SessionTokens>, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let user_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            UPDATE user_sessions
            SET revoked_at = NOW(), last_used_at = NOW()
            WHERE refresh_token_hash = $1
              AND revoked_at IS NULL
              AND refresh_expires_at > NOW()
            RETURNING user_id
            "#,
        )
        .bind(token_hash(refresh_token))
        .fetch_optional(&mut *transaction)
        .await?;

        let Some(user_id) = user_id else {
            transaction.rollback().await?;
            return Ok(None);
        };

        let active = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND status = 'active')",
        )
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !active {
            transaction.rollback().await?;
            return Ok(None);
        }

        let tokens = insert_session(&mut transaction, user_id).await?;
        transaction.commit().await?;
        Ok(Some(tokens))
    }

    pub async fn revoke_access_token(&self, access_token: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE user_sessions
            SET revoked_at = NOW()
            WHERE access_token_hash = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(token_hash(access_token))
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GoogleIdentityError {
    #[error(
        "an account already exists for this email; sign in with its existing method before linking Google"
    )]
    LinkRequired,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

async fn find_external_user(
    transaction: &mut Transaction<'_, Postgres>,
    provider: &str,
    subject: &str,
) -> Result<Option<UserRecord>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT users.id, users.display_name, users.primary_email,
               users.email_verified_at, users.created_at
        FROM user_identities
        JOIN users ON users.id = user_identities.user_id
        WHERE user_identities.provider = $1
          AND user_identities.provider_subject = $2
          AND users.status = 'active'
        "#,
    )
    .bind(provider)
    .bind(subject)
    .fetch_optional(&mut **transaction)
    .await
}

async fn insert_session(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<SessionTokens, sqlx::Error> {
    let access_token = generate_token();
    let refresh_token = generate_token();
    let now = Utc::now();
    let access_expires_at = now + Duration::minutes(ACCESS_TOKEN_MINUTES);
    let refresh_expires_at = now + Duration::days(REFRESH_TOKEN_DAYS);

    sqlx::query(
        r#"
        INSERT INTO user_sessions (
            user_id, access_token_hash, refresh_token_hash,
            access_expires_at, refresh_expires_at
        ) VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(user_id)
    .bind(token_hash(&access_token))
    .bind(token_hash(&refresh_token))
    .bind(access_expires_at)
    .bind(refresh_expires_at)
    .execute(&mut **transaction)
    .await?;

    Ok(SessionTokens {
        access_token,
        refresh_token,
        token_type: "Bearer",
        access_expires_at,
        refresh_expires_at,
    })
}
