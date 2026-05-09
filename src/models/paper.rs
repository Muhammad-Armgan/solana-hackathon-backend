use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

use crate::database::postgres::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Paper {
    pub id: Uuid,
    pub user_id: Uuid,
    pub subject: String,
    pub paper_payload: Option<Value>,
    pub download_url: Option<String>,
    pub tokens_spent: i64,
    pub created_at: DateTime<Utc>,
}

impl Paper {
    pub async fn create_generated(
        pool: &DbPool,
        user_id: Uuid,
        subject: &str,
        paper_payload: Option<Value>,
        download_url: Option<&str>,
        tokens_spent: i64,
    ) -> sqlx::Result<Self> {
        sqlx::query_as::<_, Paper>(
            r#"
            INSERT INTO papers (user_id, subject, paper_payload, download_url, tokens_spent)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, user_id, subject, paper_payload, download_url, tokens_spent, created_at
            "#,
        )
        .bind(user_id)
        .bind(subject)
        .bind(paper_payload)
        .bind(download_url)
        .bind(tokens_spent)
        .fetch_one(pool)
        .await
    }

    pub async fn find_by_id(pool: &DbPool, paper_id: Uuid) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, Paper>(
            r#"
            SELECT id, user_id, subject, paper_payload, download_url, tokens_spent, created_at
            FROM papers
            WHERE id = $1
            "#,
        )
        .bind(paper_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn history_by_user(pool: &DbPool, user_id: Uuid, limit: i64, offset: i64) -> sqlx::Result<Vec<Self>> {
        sqlx::query_as::<_, Paper>(
            r#"
            SELECT id, user_id, subject, paper_payload, download_url, tokens_spent, created_at
            FROM papers
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
