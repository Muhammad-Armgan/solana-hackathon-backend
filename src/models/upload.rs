use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::database::postgres::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Upload {
    pub id: Uuid,
    pub user_id: Uuid,
    pub filename: String,
    pub storage_path: String,
    pub status: String,
    pub ai_score: Option<f64>,
    pub reward_tokens: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Upload {
    pub async fn create(
        pool: &DbPool,
        user_id: Uuid,
        filename: &str,
        storage_path: &str,
    ) -> sqlx::Result<Self> {
        sqlx::query_as::<_, Upload>(
            r#"
            INSERT INTO uploads (user_id, filename, storage_path)
            VALUES ($1, $2, $3)
            RETURNING id, user_id, filename, storage_path, status, ai_score::double precision AS ai_score, reward_tokens, created_at, updated_at
            "#,
        )
        .bind(user_id)
        .bind(filename)
        .bind(storage_path)
        .fetch_one(pool)
        .await
    }

    pub async fn update_scoring(
        pool: &DbPool,
        upload_id: Uuid,
        status: &str,
        ai_score: Option<f64>,
        reward_tokens: i64,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, Upload>(
            r#"
            UPDATE uploads
            SET status = $2, ai_score = $3, reward_tokens = $4, updated_at = NOW()
            WHERE id = $1
            RETURNING id, user_id, filename, storage_path, status, ai_score::double precision AS ai_score, reward_tokens, created_at, updated_at
            "#,
        )
        .bind(upload_id)
        .bind(status)
        .bind(ai_score)
        .bind(reward_tokens)
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_id(pool: &DbPool, upload_id: Uuid) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, Upload>(
            r#"
            SELECT id, user_id, filename, storage_path, status, ai_score::double precision AS ai_score, reward_tokens, created_at, updated_at
            FROM uploads
            WHERE id = $1
            "#,
        )
        .bind(upload_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn history_by_user(pool: &DbPool, user_id: Uuid, limit: i64, offset: i64) -> sqlx::Result<Vec<Self>> {
        sqlx::query_as::<_, Upload>(
            r#"
            SELECT id, user_id, filename, storage_path, status, ai_score::double precision AS ai_score, reward_tokens, created_at, updated_at
            FROM uploads
            WHERE user_id = $1
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
