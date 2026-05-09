use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::database::postgres::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub wallet_address: String,
    pub email: Option<String>,
    pub signup_bonus_granted: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Balance {
    pub user_id: Uuid,
    pub token_balance: i64,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub async fn create(pool: &DbPool, wallet_address: &str, email: Option<&str>) -> sqlx::Result<Self> {
        sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (wallet_address, email)
            VALUES ($1, $2)
            RETURNING id, wallet_address, email, signup_bonus_granted, created_at, updated_at
            "#,
        )
        .bind(wallet_address)
        .bind(email)
        .fetch_one(pool)
        .await
    }

    pub async fn find_by_id(pool: &DbPool, user_id: Uuid) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, User>(
            r#"
            SELECT id, wallet_address, email, signup_bonus_granted, created_at, updated_at
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_wallet(pool: &DbPool, wallet_address: &str) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, User>(
            r#"
            SELECT id, wallet_address, email, signup_bonus_granted, created_at, updated_at
            FROM users
            WHERE wallet_address = $1
            "#,
        )
        .bind(wallet_address)
        .fetch_optional(pool)
        .await
    }

    pub async fn mark_signup_bonus_granted(pool: &DbPool, user_id: Uuid) -> sqlx::Result<()> {
        sqlx::query(
            r#"
            UPDATE users
            SET signup_bonus_granted = TRUE, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(())
    }
}

impl Balance {
    pub async fn create_if_missing(pool: &DbPool, user_id: Uuid) -> sqlx::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO balances (user_id, token_balance)
            VALUES ($1, 0)
            ON CONFLICT (user_id) DO NOTHING
            "#,
        )
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn get_by_user_id(pool: &DbPool, user_id: Uuid) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, Balance>(
            r#"
            SELECT user_id, token_balance, updated_at
            FROM balances
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn set_balance(pool: &DbPool, user_id: Uuid, new_balance: i64) -> sqlx::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO balances (user_id, token_balance, updated_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (user_id)
            DO UPDATE SET token_balance = EXCLUDED.token_balance, updated_at = NOW()
            "#,
        )
        .bind(user_id)
        .bind(new_balance)
        .execute(pool)
        .await?;

        Ok(())
    }
}
