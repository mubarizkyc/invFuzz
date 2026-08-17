use crucible_fuzzer::*;
use escrow::ID;
use sha2::{Digest, Sha256};
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::Arc;

use fuzz::{HasContext, HasInstructionSet, InstructionSpec};

const INITIAL_BALANCE: u64 = 10_000_000_000;
const UNLOCK_DELAY: u64 = 10;

const SYSTEM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0u8; 32]);

fn ix_sighash(ix_name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(b"global:");
    hasher.update(ix_name.as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[..8]);
    out
}

// Anchor account discriminator: sha256("account:{Name}")[..8].
fn account_disc(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(b"account:");
    hasher.update(name.as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[..8]);
    out
}

// ----------------------------------------------------------------------------
// raw_call ix constructors — Anchor-version-independent. AccountMeta order +
// (is_writable, is_signer) flags mirror the program's #[derive(Accounts)]
// layout for each ix. Args are borsh-serialized inline.
// ----------------------------------------------------------------------------

fn build_initialize_ix(
    program_id: Pubkey,
    vault: Pubkey,
    depositor: Pubkey,
    beneficiary: Pubkey,
    unlock_slot: u64,
) -> Instruction {
    let mut data = ix_sighash("initialize").to_vec();
    data.extend_from_slice(beneficiary.as_ref());
    data.extend_from_slice(&unlock_slot.to_le_bytes());
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(vault, false),
            AccountMeta::new(depositor, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data,
    }
}

fn build_deposit_ix(
    program_id: Pubkey,
    vault: Pubkey,
    depositor: Pubkey,
    amount: u64,
) -> Instruction {
    let mut data = ix_sighash("deposit").to_vec();
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(vault, false),
            AccountMeta::new(depositor, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data,
    }
}

fn build_withdraw_ix(
    program_id: Pubkey,
    vault: Pubkey,
    depositor: Pubkey,
    amount: u64,
) -> Instruction {
    let mut data = ix_sighash("withdraw").to_vec();
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(vault, false),
            AccountMeta::new(depositor, true),
        ],
        data,
    }
}
#[derive(Clone)]
struct EscrowFixture {
    pub ctx: TestContext,
    program_id: Pubkey,
    depositor: Arc<Keypair>, // primary trader / used by all action_*
    fee_payer: Arc<Keypair>,
    vault_pda: Pubkey, // primary vault (depositor's)
    successful_withdraw_slots: Vec<u64>,
    unlock_slot: u64,
}

#[fuzz_fixture]
impl EscrowFixture {
    pub fn setup() -> Self {
        let mut ctx = TestContext::new();
        let program_id = Pubkey::new_from_array(ID.to_bytes());
        ctx.add_program(&program_id, "../../target/deploy/escrow.so")
            .unwrap();

        let depositor = Arc::new(Keypair::new());
        let depositor_b = Arc::new(Keypair::new());
        let beneficiary = Arc::new(Keypair::new());
        let fee_payer = Arc::new(Keypair::new());
        for kp in [&depositor, &depositor_b, &beneficiary, &fee_payer] {
            ctx.create_account()
                .pubkey(kp.pubkey())
                .lamports(INITIAL_BALANCE)
                .owner(SYSTEM_PROGRAM_ID)
                .create()
                .unwrap();
        }

        let (vault_pda, _) =
            Pubkey::find_program_address(&[b"vault", depositor.pubkey().as_ref()], &program_id);
        let (vault_b_pda, _) =
            Pubkey::find_program_address(&[b"vault", depositor_b.pubkey().as_ref()], &program_id);
        let unlock_slot = ctx.slot() + UNLOCK_DELAY;

        ctx.raw_call(build_initialize_ix(
            program_id,
            vault_pda,
            depositor.pubkey(),
            beneficiary.pubkey(),
            unlock_slot,
        ))
        .signers(&[&*depositor])
        .send()
        .unwrap();

        ctx.raw_call(build_initialize_ix(
            program_id,
            vault_b_pda,
            depositor_b.pubkey(),
            beneficiary.pubkey(),
            unlock_slot,
        ))
        .signers(&[&*depositor_b])
        .send()
        .unwrap();

        ctx.raw_call(build_deposit_ix(
            program_id,
            vault_b_pda,
            depositor_b.pubkey(),
            5_000_000,
        ))
        .signers(&[&*depositor_b])
        .send()
        .unwrap();

        Self {
            ctx,
            program_id,
            depositor,
            fee_payer,
            vault_pda,
            unlock_slot,
            successful_withdraw_slots: Vec::new(),
        }
    }

    pub fn action_deposit(&mut self, #[range(1..1_000_000)] amount: u64) -> bool {
        self.ctx
            .raw_call(build_deposit_ix(
                self.program_id,
                self.vault_pda,
                self.depositor.pubkey(),
                amount,
            ))
            .signers(&[&*self.depositor])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    pub fn action_withdraw(&mut self, #[range(1..1_000_000)] amount: u64) -> bool {
        let pre_slot = self.ctx.slot();
        let success = self
            .ctx
            .raw_call(build_withdraw_ix(
                self.program_id,
                self.vault_pda,
                self.depositor.pubkey(),
                amount,
            ))
            .signers(&[&*self.depositor])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if success {
            self.successful_withdraw_slots.push(pre_slot);
        }
        success
    }

    pub fn action_advance_slots(&mut self, #[range(0..15)] slots: u64) -> bool {
        let new_slot = self.ctx.slot() + slots;
        self.ctx.warp_to_slot(new_slot);
        true
    }
}

impl HasContext for EscrowFixture {
    fn ctx(&self) -> &TestContext {
        &self.ctx
    }
    fn ctx_mut(&mut self) -> &mut TestContext {
        &mut self.ctx
    }
    fn program_ids(&self) -> Vec<Pubkey> {
        vec![self.program_id]
    }
    fn fee_payer(&self) -> Arc<Keypair> {
        Arc::clone(&self.fee_payer)
    }
}

impl HasInstructionSet for EscrowFixture {
    fn instructions(&self) -> Vec<InstructionSpec> {
        let mut data = withdraw_sighash().to_vec();
        data.extend_from_slice(&100_000u64.to_le_bytes());

        let withdraw_spec = InstructionSpec {
            program_id: self.program_id,
            name: "withdraw".into(),
            accounts: vec![
                AccountMeta::new(self.vault_pda, false),         // vault
                AccountMeta::new(self.depositor.pubkey(), true), // depositor (signer)
            ],
            expected_owners: vec![Some(self.program_id), None],
            expected_discriminators: vec![Some(vault_discriminator()), None],
            expected_pda_seeds: vec![
                Some(vec![
                    b"vault".to_vec(),
                    self.depositor.pubkey().to_bytes().to_vec(),
                ]),
                None,
            ],
            data_sample: data,
            signers: vec![Arc::clone(&self.depositor)],
        };
        vec![withdraw_spec]
    }
}
//deterministic
fn withdraw_sighash() -> [u8; 8] {
    ix_sighash("withdraw")
}
fn vault_discriminator() -> [u8; 8] {
    account_disc("Vault")
}

#[invariant_test]
fn invariant_acceptance(fixture: &EscrowFixture) {
    invariants::invariants::discriminator_skip::check(fixture);
}

#[invariant_test]
fn invariant_discriminator_skip_only(fixture: &EscrowFixture) {
    invariants::invariants::discriminator_skip::check(fixture);
}
