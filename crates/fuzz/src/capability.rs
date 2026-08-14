// User fixtures implement these traits in addition to Crucible's
// `#[fuzz_fixture]` requirements, allowing invariants to
// introspect the instruction set + program registry without competing
// with Crucible's macro vocabulary.

use crucible_test_context::TestContext;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use std::sync::Arc;

pub trait HasContext {
    fn ctx(&self) -> &TestContext;
    fn ctx_mut(&mut self) -> &mut TestContext;
    fn program_ids(&self) -> Vec<Pubkey>;
    fn fee_payer(&self) -> Arc<Keypair>;
}
pub trait HasInstructionSet {
    fn instructions(&self) -> Vec<InstructionSpec>;
}

/// Per-instruction specification declaring metadata for invariant detectors.

#[derive(Clone)]
pub struct InstructionSpec {
    pub program_id: Pubkey,
    pub name: String,
    pub accounts: Vec<AccountMeta>,
    pub expected_owners: Vec<Option<Pubkey>>,
    pub expected_discriminators: Vec<Option<[u8; 8]>>,
    pub expected_pda_seeds: Vec<Option<Vec<Vec<u8>>>>,
    pub data_sample: Vec<u8>,
    pub signers: Vec<Arc<Keypair>>,
}

impl InstructionSpec {
    pub fn to_instruction(&self) -> Instruction {
        Instruction {
            program_id: self.program_id,
            accounts: self.accounts.clone(),
            data: self.data_sample.clone(),
        }
    }
}
