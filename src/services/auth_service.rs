use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{config::Settings, utils::error::AppError, utils::signature::verify_wallet_signature};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: Uuid,
    pub wallet_address: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Clone)]
pub struct AuthService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    jwt_expiry_hours: i64,
}

impl AuthService {
    pub fn new(settings: &Settings) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(settings.jwt_secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(settings.jwt_secret.as_bytes()),
            jwt_expiry_hours: settings.jwt_expiry_hours,
        }
    }

    pub fn create_login_challenge(&self, wallet_address: &str) -> String {
        let nonce: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(24)
            .map(char::from)
            .collect();

        format!(
            "Sign this message to authenticate with Universal Learning Platform.\\nWallet: {}\\nNonce: {}",
            wallet_address, nonce
        )
    }

    pub fn verify_wallet_login(
        &self,
        wallet_address: &str,
        signed_message: &str,
        signature_base58: &str,
    ) -> Result<(), AppError> {
        verify_wallet_signature(wallet_address, signed_message, signature_base58)
    }

    pub fn issue_jwt(&self, user_id: Uuid, wallet_address: &str) -> Result<String, AppError> {
        let issued_at = Utc::now();
        let expires_at = issued_at + Duration::hours(self.jwt_expiry_hours);

        let claims = AuthClaims {
            sub: user_id,
            wallet_address: wallet_address.to_string(),
            exp: expires_at.timestamp() as usize,
            iat: issued_at.timestamp() as usize,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|_| AppError::Internal)
    }

    pub fn decode_jwt(&self, token: &str) -> Result<AuthClaims, AppError> {
        let mut validation = Validation::default();
        validation.validate_exp = true;

        decode::<AuthClaims>(token, &self.decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|_| AppError::Unauthorized("Invalid or expired token".to_string()))
    }
}
