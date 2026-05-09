use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

use crate::database::postgres::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Quiz {
    pub id: Uuid,
    pub user_id: Uuid,
    pub subject: String,
    pub questions: Value,
    pub answers: Option<Value>,
    pub score: Option<f64>,
    pub tokens_spent: i64,
    pub created_at: DateTime<Utc>,
}

impl Quiz {
    pub async fn find_by_id(pool: &DbPool, quiz_id: Uuid) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, Quiz>(
            r#"
            SELECT id, user_id, subject, questions, answers, score::double precision AS score, tokens_spent, created_at
            FROM quizzes
            WHERE id = $1
            "#,
        )
        .bind(quiz_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn create_generated(
        pool: &DbPool,
        user_id: Uuid,
        subject: &str,
        questions: Value,
        tokens_spent: i64,
    ) -> sqlx::Result<Self> {
        sqlx::query_as::<_, Quiz>(
            r#"
            INSERT INTO quizzes (user_id, subject, questions, tokens_spent)
            VALUES ($1, $2, $3, $4)
            RETURNING id, user_id, subject, questions, answers, score::double precision AS score, tokens_spent, created_at
            "#,
        )
        .bind(user_id)
        .bind(subject)
        .bind(questions)
        .bind(tokens_spent)
        .fetch_one(pool)
        .await
    }

    pub async fn submit_answers(
        pool: &DbPool,
        quiz_id: Uuid,
        answers: Value,
        score: f64,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, Quiz>(
            r#"
            UPDATE quizzes
            SET answers = $2, score = $3
            WHERE id = $1
            RETURNING id, user_id, subject, questions, answers, score::double precision AS score, tokens_spent, created_at
            "#,
        )
        .bind(quiz_id)
        .bind(answers)
        .bind(score)
        .fetch_optional(pool)
        .await
    }

    pub async fn history_by_user(pool: &DbPool, user_id: Uuid, limit: i64, offset: i64) -> sqlx::Result<Vec<Self>> {
        sqlx::query_as::<_, Quiz>(
            r#"
            SELECT id, user_id, subject, questions, answers, score::double precision AS score, tokens_spent, created_at
            FROM quizzes
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
