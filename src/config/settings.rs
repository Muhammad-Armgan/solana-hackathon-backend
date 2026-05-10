use std::{env, path::PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Settings {
    pub database_url: String,
    pub database_max_connections: u32,
    pub database_min_connections: u32,
    pub database_connect_timeout_seconds: u64,
    pub redis_url: String,
    pub redis_max_connections: u32,
    pub jwt_secret: String,
    pub jwt_expiry_hours: i64,
    pub storage_path: PathBuf,
    pub cloudinary_cloud_name: Option<String>,
    pub cloudinary_api_key: Option<String>,
    pub cloudinary_api_secret: Option<String>,
    pub solana_rpc_url: String,
    pub solana_wallet_private_key: String,
    pub solana_token_mint_address: String,
    pub solana_program_id: String,
    pub hf_quiz_api_url: String,
    pub hf_paper_api_url: String,
    pub hf_score_api_url: String,
    pub hf_api_token: Option<String>,
    pub frontend_url: String,
    pub max_upload_size: usize,
    pub rate_limit_quizzes_per_day: u64,
    pub rate_limit_papers_per_day: u64,
    pub rate_limit_uploads_per_day: u64,
    pub quiz_cooldown_seconds: u64,
}

impl Settings {
    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();

        Ok(Self {
            database_url: env::var("DATABASE_URL")
                .context("DATABASE_URL is not set. Please configure it in your .env file")?,
            database_max_connections: env::var("DATABASE_MAX_CONNECTIONS").ok().and_then(|v| v.parse().ok()).unwrap_or(5),
            database_min_connections: env::var("DATABASE_MIN_CONNECTIONS").ok().and_then(|v| v.parse().ok()).unwrap_or(1),
            database_connect_timeout_seconds: env::var("DATABASE_CONNECT_TIMEOUT_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(10),
            redis_url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            redis_max_connections: env::var("REDIS_MAX_CONNECTIONS").ok().and_then(|v| v.parse().ok()).unwrap_or(20),
            jwt_secret: env::var("JWT_SECRET").context("JWT_SECRET is not set. Please configure it in your .env file")?,
            jwt_expiry_hours: env::var("JWT_EXPIRY_HOURS").ok().and_then(|v| v.parse().ok()).unwrap_or(24),
            storage_path: env::var("STORAGE_PATH").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("./uploads")),
            cloudinary_cloud_name: env::var("CLOUDINARY_CLOUD_NAME").ok(),
            cloudinary_api_key: env::var("CLOUDINARY_API_KEY").ok(),
            cloudinary_api_secret: env::var("CLOUDINARY_API_SECRET").ok(),
            solana_rpc_url: env::var("SOLANA_RPC_URL").unwrap_or_else(|_| "https://api.devnet.solana.com".to_string()),
            solana_wallet_private_key: env::var("SOLANA_WALLET_PRIVATE_KEY").unwrap_or_default(),
            solana_token_mint_address: env::var("SOLANA_TOKEN_MINT_ADDRESS").unwrap_or_default(),
            solana_program_id: env::var("SOLANA_PROGRAM_ID").unwrap_or_default(),
            hf_quiz_api_url: env::var("HF_QUIZ_API_URL").unwrap_or_default(),
            hf_paper_api_url: env::var("HF_PAPER_API_URL").unwrap_or_default(),
            hf_score_api_url: env::var("HF_SCORE_API_URL").unwrap_or_default(),
            hf_api_token: env::var("HF_API_TOKEN").ok(),
            frontend_url: env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3001".to_string()),
            max_upload_size: env::var("MAX_UPLOAD_SIZE").ok().and_then(|v| v.parse().ok()).unwrap_or(10 * 1024 * 1024),
            rate_limit_quizzes_per_day: env::var("RATE_LIMIT_QUIZZES_PER_DAY").ok().and_then(|v| v.parse().ok()).unwrap_or(20),
            rate_limit_papers_per_day: env::var("RATE_LIMIT_PAPERS_PER_DAY").ok().and_then(|v| v.parse().ok()).unwrap_or(10),
            rate_limit_uploads_per_day: env::var("RATE_LIMIT_UPLOADS_PER_DAY").ok().and_then(|v| v.parse().ok()).unwrap_or(5),
            quiz_cooldown_seconds: env::var("QUIZ_COOLDOWN_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(30),
        })
    }
}
