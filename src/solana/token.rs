use anyhow::Result;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::solana::{client::SolanaClient, instructions};

#[derive(Clone)]
pub struct TokenService {
    client: SolanaClient,
}

impl TokenService {
    pub fn new(client: SolanaClient) -> Self {
        Self { client }
    }

    /// Expose the underlying RPC client for direct use in handlers.
    pub fn client(&self) -> &SolanaClient {
        &self.client
    }

    /// Derive the Associated Token Account address for a given wallet.
    pub fn associated_token_address(&self, owner: &Pubkey) -> Pubkey {
        spl_associated_token_account::get_associated_token_address_with_program_id(
            owner,
            &self.client.mint,
            &spl_token::id(),
        )
    }

    /// Get the on-chain COIN balance for a wallet (in raw units, 2 decimals).
    /// Returns 0 if the ATA doesn't exist yet.
    pub async fn get_token_balance(&self, owner: &Pubkey) -> Result<u64> {
        let ata = self.associated_token_address(owner);
        if !self.client.account_exists(&ata).await? {
            return Ok(0);
        }
        self.client.get_token_account_balance(&ata).await
    }

    /// Get the on-chain COIN balance for a wallet (in COIN units, not raw).
    /// Returns 0 if the ATA doesn't exist yet.
    pub async fn get_coin_balance(&self, owner: &Pubkey) -> Result<i64> {
        let raw = self.get_token_balance(owner).await?;
        // raw units / 100 = COIN (2 decimals)
        Ok((raw / 100) as i64)
    }

    /// Mint COIN tokens to a user's wallet.
    ///
    /// The platform wallet is the mint authority, so this works server-side.
    /// Used for: signup bonus, upload rewards, buy credits.
    ///
    /// `amount` is in raw units (1 COIN = 100 raw units with 2 decimals).
    pub async fn mint_tokens_to_user(&self, user_wallet: &Pubkey, amount: u64) -> Result<String> {
        let destination_ata = self.associated_token_address(user_wallet);
        let mut ixs = Vec::new();

        // Create the ATA if it doesn't exist yet
        if !self.client.account_exists(&destination_ata).await? {
            tracing::info!(
                "Creating ATA {} for wallet {}",
                destination_ata,
                user_wallet
            );
            ixs.push(
                spl_associated_token_account::instruction::create_associated_token_account(
                    &self.client.payer.pubkey(),
                    user_wallet,
                    &self.client.mint,
                    &spl_token::id(),
                ),
            );
        }

        ixs.push(instructions::build_mint_to_instruction(
            &self.client.mint,
            &destination_ata,
            &self.client.payer.pubkey(), // payer is mint authority
            amount,
        )?);

        let blockhash = self.client.get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&self.client.payer.pubkey()),
            &[self.client.payer.as_ref()],
            blockhash,
        );

        let sig = self.client.send_transaction(&tx).await?;
        tracing::info!("mint_tokens_to_user: {} raw units → {} tx={}", amount, user_wallet, sig);
        Ok(sig)
    }

    /// Transfer COIN between two wallets using the platform as a relay:
    ///   1. Mint `amount` to the recipient (server can do this — it's mint authority)
    ///   2. The sender's on-chain balance is NOT reduced here because the server
    ///      doesn't hold the sender's private key. The DB balance is the source
    ///      of truth for deductions.
    ///
    /// For a fully trustless transfer the user would sign on the frontend.
    /// This custodial approach is acceptable for a hackathon / MVP.
    pub async fn custodial_transfer(
        &self,
        recipient_wallet: &Pubkey,
        amount: u64,
    ) -> Result<String> {
        self.mint_tokens_to_user(recipient_wallet, amount).await
    }

    /// Burn tokens from a user's wallet.
    /// Requires the user's keypair — only usable when the server holds it.
    pub async fn burn_tokens_from_user(
        &self,
        user_owner: &Keypair,
        amount: u64,
    ) -> Result<String> {
        let source_ata = self.associated_token_address(&user_owner.pubkey());
        let burn_ix = instructions::build_burn_instruction(
            &self.client.mint,
            &source_ata,
            &user_owner.pubkey(),
            amount,
        )?;

        let blockhash = self.client.get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &[burn_ix],
            Some(&user_owner.pubkey()),
            &[user_owner],
            blockhash,
        );

        self.client.send_transaction(&tx).await
    }

    /// Full SPL transfer between two wallets.
    /// Requires the sender's keypair — only usable when the server holds it.
    pub async fn transfer_tokens(
        &self,
        sender_owner: &Keypair,
        recipient_wallet: &Pubkey,
        amount: u64,
    ) -> Result<String> {
        let source_ata = self.associated_token_address(&sender_owner.pubkey());
        let destination_ata = self.associated_token_address(recipient_wallet);

        let mut ixs = Vec::new();

        if !self.client.account_exists(&destination_ata).await? {
            ixs.push(
                spl_associated_token_account::instruction::create_associated_token_account(
                    &self.client.payer.pubkey(),
                    recipient_wallet,
                    &self.client.mint,
                    &spl_token::id(),
                ),
            );
        }

        ixs.push(instructions::build_transfer_instruction(
            &source_ata,
            &destination_ata,
            &sender_owner.pubkey(),
            amount,
        )?);

        let blockhash = self.client.get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&self.client.payer.pubkey()),
            &[self.client.payer.as_ref(), sender_owner],
            blockhash,
        );

        self.client.send_transaction(&tx).await
    }
}
