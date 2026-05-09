use anyhow::{Context, Result};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};

pub fn build_mint_to_instruction(
    mint: &Pubkey,
    destination_ata: &Pubkey,
    mint_authority: &Pubkey,
    amount: u64,
) -> Result<Instruction> {
    spl_token::instruction::mint_to(
        &spl_token::id(),
        mint,
        destination_ata,
        mint_authority,
        &[],
        amount,
    )
    .context("Failed to build mint_to instruction")
}

pub fn build_burn_instruction(
    mint: &Pubkey,
    source_ata: &Pubkey,
    owner: &Pubkey,
    amount: u64,
) -> Result<Instruction> {
    spl_token::instruction::burn(
        &spl_token::id(),
        source_ata,
        mint,
        owner,
        &[],
        amount,
    )
    .context("Failed to build burn instruction")
}

pub fn build_transfer_instruction(
    source_ata: &Pubkey,
    destination_ata: &Pubkey,
    owner: &Pubkey,
    amount: u64,
) -> Result<Instruction> {
    spl_token::instruction::transfer(
        &spl_token::id(),
        source_ata,
        destination_ata,
        owner,
        &[],
        amount,
    )
    .context("Failed to build transfer instruction")
}
