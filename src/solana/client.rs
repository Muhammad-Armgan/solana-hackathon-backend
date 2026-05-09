use std::str::FromStr;

use anyhow::{Context, Result};
use base64::Engine;
use serde::Deserialize;
use solana_sdk::{
    hash::Hash,
    pubkey::Pubkey,
    signature::Keypair,
    transaction::Transaction,
};

use crate::config::Settings;

#[derive(Clone)]
pub struct SolanaClient {
    pub rpc_url: String,
    pub http: reqwest::Client,
    pub payer: std::sync::Arc<Keypair>,
    pub mint: Pubkey,
    pub program_id: Pubkey,
}

impl SolanaClient {
    pub fn from_settings(settings: &Settings) -> Result<Self> {
        let payer = decode_keypair_from_base58(&settings.solana_wallet_private_key)
            .context("Failed to decode SOLANA_WALLET_PRIVATE_KEY")?;

        let mint = Pubkey::from_str(&settings.solana_token_mint_address)
            .context("Invalid SOLANA_TOKEN_MINT_ADDRESS")?;

        let program_id = Pubkey::from_str(&settings.solana_program_id)
            .context("Invalid SOLANA_PROGRAM_ID")?;

        Ok(Self {
            rpc_url: settings.solana_rpc_url.clone(),
            http: reqwest::Client::new(),
            payer: std::sync::Arc::new(payer),
            mint,
            program_id,
        })
    }

    pub async fn get_latest_blockhash(&self) -> Result<Hash> {
        let raw = self
            .rpc_call_raw("getLatestBlockhash", serde_json::json!([{"commitment": "confirmed"}]))
            .await?;

        let blockhash = raw["result"]["value"]["blockhash"]
            .as_str()
            .context("Missing blockhash in RPC response")?;

        blockhash.parse::<Hash>().context("Invalid blockhash")
    }

    /// Send a signed transaction and return the transaction signature.
    pub async fn send_transaction(&self, tx: &Transaction) -> Result<String> {
        let serialized = bincode::serialize(tx).context("Failed to serialize transaction")?;
        let tx_base64 = base64::engine::general_purpose::STANDARD.encode(serialized);

        let raw = self
            .rpc_call_raw(
                "sendTransaction",
                serde_json::json!([
                    tx_base64,
                    {
                        "encoding": "base64",
                        "skipPreflight": false,
                        "preflightCommitment": "confirmed"
                    }
                ]),
            )
            .await?;

        // Check for RPC-level error first
        if let Some(err) = raw.get("error") {
            let code = err["code"].as_i64().unwrap_or(0);
            let msg = err["message"].as_str().unwrap_or("unknown RPC error");
            let data = err.get("data");
            tracing::error!(
                "sendTransaction RPC error code={} msg={} data={:?}",
                code, msg, data
            );
            anyhow::bail!("RPC error {}: {}", code, msg);
        }

        // Success — result is the transaction signature string
        let sig = raw["result"]
            .as_str()
            .context("sendTransaction: result is not a string")?;

        Ok(sig.to_string())
    }

    pub async fn get_token_account_balance(&self, token_account: &Pubkey) -> Result<u64> {
        let raw = self
            .rpc_call_raw(
                "getTokenAccountBalance",
                serde_json::json!([token_account.to_string(), {"commitment": "confirmed"}]),
            )
            .await?;

        let amount = raw["result"]["value"]["amount"]
            .as_str()
            .context("Missing amount in token balance response")?;

        amount.parse::<u64>().context("Invalid token balance amount")
    }

    pub async fn account_exists(&self, account: &Pubkey) -> Result<bool> {
        let raw = self
            .rpc_call_raw(
                "getAccountInfo",
                serde_json::json!([account.to_string(), {"encoding": "base64", "commitment": "confirmed"}]),
            )
            .await?;

        Ok(!raw["result"].is_null() && !raw["result"]["value"].is_null())
    }

    /// Low-level RPC call — returns the raw JSON value so callers can
    /// inspect both `result` and `error` fields.
    async fn rpc_call_raw(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let resp = self
            .http
            .post(&self.rpc_url)
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("HTTP request failed for RPC method {method}"))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_else(|_| "<no body>".to_string());
            anyhow::bail!("RPC HTTP {}: {}", method, body);
        }

        let value: serde_json::Value = resp
            .json()
            .await
            .with_context(|| format!("Failed to parse JSON for RPC method {method}"))?;

        tracing::debug!("RPC {} response: {}", method, value);

        Ok(value)
    }
}

fn decode_keypair_from_base58(private_key_base58: &str) -> Result<Keypair> {
    let secret_bytes = bs58::decode(private_key_base58)
        .into_vec()
        .context("Base58 decode failed for SOLANA_WALLET_PRIVATE_KEY")?;

    Keypair::try_from(secret_bytes.as_slice())
        .context("Keypair parse failed — ensure SOLANA_WALLET_PRIVATE_KEY is a 64-byte keypair in base58")
}

// Keep these for any code that still uses the typed structs
#[derive(Deserialize)]
pub struct RpcResponse<T> {
    pub result: T,
}

#[derive(Deserialize)]
pub struct RpcLatestBlockhashValue {
    pub value: RpcBlockhash,
}

#[derive(Deserialize)]
pub struct RpcBlockhash {
    pub blockhash: String,
}

#[derive(Deserialize)]
pub struct RpcTokenBalanceValue {
    pub value: RpcTokenAmount,
}

#[derive(Deserialize)]
pub struct RpcTokenAmount {
    pub amount: String,
}
