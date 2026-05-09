use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::Settings;

#[derive(Clone)]
pub struct AiClient {
    client: reqwest::Client,
    quiz_url: String,
    paper_url: String,
    score_url: String,
    token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadScoreResult {
    pub ai_score: f64,
    pub reward_tokens: i64,
}

impl AiClient {
    pub fn new(settings: &Settings) -> Self {
        Self {
            client: reqwest::Client::new(),
            quiz_url: settings.hf_quiz_api_url.clone(),
            paper_url: settings.hf_paper_api_url.clone(),
            score_url: settings.hf_score_api_url.clone(),
            token: settings.hf_api_token.clone(),
        }
    }

    pub async fn generate_quiz(&self, subject: &str) -> Value {
        let payload = json!({"subject": subject, "count": 10});
        if let Ok(v) = self.post_json(&self.quiz_url, payload).await { return v; }

        // Fallback static quiz when AI service is unavailable.
        json!({"questions": [
            {"q": format!("Sample {} question 1", subject), "options": ["A","B","C","D"], "answer": "A"},
            {"q": format!("Sample {} question 2", subject), "options": ["A","B","C","D"], "answer": "B"}
        ]})
    }

    pub async fn generate_paper(&self, subject: &str) -> Value {
        let payload = json!({"subject": subject, "format": "practice"});
        if let Ok(v) = self.post_json(&self.paper_url, payload).await { return v; }

        // Fallback static paper payload.
        json!({"title": format!("{} Practice Paper", subject), "sections": ["MCQ", "Short Questions"]})
    }

    pub async fn score_upload(&self, file_name: &str) -> UploadScoreResult {
        let payload = json!({"filename": file_name});
        if let Ok(v) = self.post_json(&self.score_url, payload).await {
            let ai_score = v.get("ai_score").and_then(|x| x.as_f64()).unwrap_or(70.0);
            let reward_tokens = v.get("reward_tokens").and_then(|x| x.as_i64()).unwrap_or(20).clamp(0, 50);
            return UploadScoreResult { ai_score, reward_tokens };
        }

        // Fallback score/reward when AI scoring API is unreachable.
        UploadScoreResult { ai_score: 72.5, reward_tokens: 20 }
    }

    async fn post_json(&self, url: &str, payload: Value) -> Result<Value> {
        if url.is_empty() {
            anyhow::bail!("empty endpoint");
        }

        let mut req = self.client.post(url).json(&payload);
        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }

        let res = req.send().await?;
        if !res.status().is_success() {
            anyhow::bail!("non-success status");
        }
        Ok(res.json().await?)
    }
}
