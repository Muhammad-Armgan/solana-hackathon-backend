use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use crate::utils::error::AppError;

/// Verify a Phantom wallet signature.
///
/// Phantom's `signMessage(bytes, "utf8")` prepends a Solana-specific prefix
/// before signing:
///
///   "\x19Solana Signed Message:\n" + varint(len(message)) + message_bytes
///
/// We try verification with the prefix first, then fall back to the raw
/// message bytes (useful for testing with manually-crafted signatures).
pub fn verify_wallet_signature(
    wallet_address_base58: &str,
    signed_message: &str,
    signature_base58: &str,
) -> Result<(), AppError> {
    // --- Decode public key ---
    let pubkey_bytes = bs58::decode(wallet_address_base58)
        .into_vec()
        .map_err(|_| AppError::BadRequest("Invalid wallet address encoding".to_string()))?;

    if pubkey_bytes.len() != 32 {
        return Err(AppError::BadRequest(
            "Wallet address must decode to 32 bytes".to_string(),
        ));
    }

    let verifying_key = VerifyingKey::from_bytes(
        pubkey_bytes
            .as_slice()
            .try_into()
            .map_err(|_| AppError::BadRequest("Invalid wallet public key bytes".to_string()))?,
    )
    .map_err(|_| AppError::BadRequest("Failed to parse wallet public key".to_string()))?;

    // --- Decode signature ---
    let signature_bytes = bs58::decode(signature_base58)
        .into_vec()
        .map_err(|_| AppError::BadRequest("Invalid signature encoding".to_string()))?;

    if signature_bytes.len() != 64 {
        return Err(AppError::BadRequest(
            "Signature must decode to 64 bytes".to_string(),
        ));
    }

    let signature = Signature::from_bytes(
        signature_bytes
            .as_slice()
            .try_into()
            .map_err(|_| AppError::BadRequest("Invalid signature bytes".to_string()))?,
    );

    let message_bytes = signed_message.as_bytes();

    // --- Attempt 1: Phantom-prefixed message ---
    // Phantom prepends "\x19Solana Signed Message:\n" + varint(len) before signing.
    let prefixed = build_solana_prefixed_message(message_bytes);
    if verifying_key.verify(&prefixed, &signature).is_ok() {
        return Ok(());
    }

    // --- Attempt 2: Raw message bytes (fallback / testing) ---
    verifying_key
        .verify(message_bytes, &signature)
        .map_err(|_| {
            AppError::Unauthorized(
                "Wallet signature verification failed. \
                 Make sure you are signing with the correct Phantom wallet."
                    .to_string(),
            )
        })?;

    Ok(())
}

/// Build the byte array that Phantom actually signs when you call
/// `signMessage(messageBytes, "utf8")`.
///
/// Format:
///   b"\x19Solana Signed Message:\n"  (26 bytes)
///   + encode_varint(message_len)
///   + message_bytes
fn build_solana_prefixed_message(message: &[u8]) -> Vec<u8> {
    const PREFIX: &[u8] = b"\x19Solana Signed Message:\n";

    let mut buf = Vec::with_capacity(PREFIX.len() + 4 + message.len());
    buf.extend_from_slice(PREFIX);

    // Encode message length as a compact varint (same as Borsh/Solana encoding)
    let mut len = message.len() as u64;
    loop {
        let mut byte = (len & 0x7F) as u8;
        len >>= 7;
        if len != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if len == 0 {
            break;
        }
    }

    buf.extend_from_slice(message);
    buf
}
