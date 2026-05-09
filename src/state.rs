use crate::{
    config::Settings,
    database::postgres::DbPool,
    services::{ai_client::AiClient, auth_service::AuthService, file_storage::StorageProvider},
    solana::token::TokenService,
};

#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
    pub db: DbPool,
    /// None when Redis is unavailable — rate limiting is skipped in that case.
    pub redis: Option<redis::aio::ConnectionManager>,
    pub auth: AuthService,
    pub ai_client: AiClient,
    pub storage: StorageProvider,
    pub solana: Option<TokenService>,
}

pub type SharedState = std::sync::Arc<AppState>;
