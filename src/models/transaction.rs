use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::database::postgres::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Transaction {
    pub id: Uuid,
    pub from_user_id: Option<Uuid>,
    pub to_user_id: Option<Uuid>,
    pub amount: i64,
    pub tx_type: String,
    pub reference_id: Option<Uuid>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TokenTransfer {
    pub id: Uuid,
    pub sender_user_id: Uuid,
    pub recipient_user_id: Uuid,
    pub amount: i64,
    pub transaction_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl Transaction {
    pub async fn create(
        pool: &DbPool,
        from_user_id: Option<Uuid>,
        to_user_id: Option<Uuid>,
        amount: i64,
        tx_type: &str,
        reference_id: Option<Uuid>,
        note: Option<&str>,
    ) -> sqlx::Result<Self> {
        sqlx::query_as::<_, Transaction>(
            r#"
            INSERT INTO transactions (from_user_id, to_user_id, amount, tx_type, reference_id, note)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, from_user_id, to_user_id, amount, tx_type, reference_id, note, created_at
            "#,
        )
        .bind(from_user_id)
        .bind(to_user_id)
        .bind(amount)
        .bind(tx_type)
        .bind(reference_id)
        .bind(note)
        .fetch_one(pool)
        .await
    }

    pub async fn history_by_user(pool: &DbPool, user_id: Uuid, limit: i64, offset: i64) -> sqlx::Result<Vec<Self>> {
        sqlx::query_as::<_, Transaction>(
            r#"
            SELECT id, from_user_id, to_user_id, amount, tx_type, reference_id, note, created_at
            FROM transactions
            WHERE from_user_id = $1 OR to_user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }
}

impl TokenTransfer {
    pub async fn create(
        pool: &DbPool,
        sender_user_id: Uuid,
        recipient_user_id: Uuid,
        amount: i64,
        transaction_id: Option<Uuid>,
    ) -> sqlx::Result<Self> {
        sqlx::query_as::<_, TokenTransfer>(
            r#"
            INSERT INTO token_transfers (sender_user_id, recipient_user_id, amount, transaction_id)
            VALUES ($1, $2, $3, $4)
            RETURNING id, sender_user_id, recipient_user_id, amount, transaction_id, created_at
            "#,
        )
        .bind(sender_user_id)
        .bind(recipient_user_id)
        .bind(amount)
        .bind(transaction_id)
        .fetch_one(pool)
        .await
    }

    pub async fn send_receive_history_by_user(
        pool: &DbPool,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> sqlx::Result<Vec<Self>> {
        sqlx::query_as::<_, TokenTransfer>(
            r#"
            SELECT id, sender_user_id, recipient_user_id, amount, transaction_id, created_at
            FROM token_transfers
            WHERE sender_user_id = $1 OR recipient_user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }
}
