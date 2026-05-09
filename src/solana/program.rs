use anyhow::Result;
use solana_sdk::pubkey::Pubkey;

use crate::solana::client::SolanaClient;

pub struct ProgramClient {
    pub program_id: Pubkey,
}

impl ProgramClient {
    pub fn from_client(client: &SolanaClient) -> Self {
        Self {
            program_id: client.program_id,
        }
    }

    pub fn program_id(&self) -> Pubkey {
        self.program_id
    }

    pub fn ensure_program_available(&self) -> Result<()> {
        // Anchor instruction wrappers will be added in Step 8.
        Ok(())
    }
}
