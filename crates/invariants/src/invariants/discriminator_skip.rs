//! # Invariant: discriminator-skip

//! For each instruction with declared `expected_discriminators`,
//! substitutes a fake account containing identical bytes EXCEPT the
//! first 8 bytes (discriminator), keeping the owner correct. If the
//! instruction succeeds and state changes, report discriminator-skip

use crucible_test_context::{fuzz_assert, TxOutcome};
use fuzz::{HasContext, HasInstructionSet};
use solana_account::Account;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;

use super::util::{
    cleanup_temp_account, hash_accounts, hash_accounts_now, restore_accounts, save_accounts,
};

const WRONG_DISCRIMINATORS: &[[u8; 8]] = &[
    [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00, 0x00],
    [0x00; 8],
    [0xFF; 8],
];

/// Check fixture's instruction set for discriminator-skip vulnerabilities.
/// For each instruction × discriminator-checked account index, runs  multi-pass substitution with the 3 wrong-discriminator candidates.
/// First pass to succeed reports violation.
pub fn check<F>(fixture: &mut F)
where
    F: HasContext + HasInstructionSet,
{
    let ixs = fixture.instructions();
    for spec in &ixs {
        for (idx, expected) in spec.expected_discriminators.iter().enumerate() {
            let Some(expected_disc) = expected else {
                continue;
            };

            for wrong_disc in WRONG_DISCRIMINATORS {
                if wrong_disc == expected_disc {
                    continue;
                }
                run_attempt(fixture, spec, idx, *wrong_disc, *expected_disc);
            }
        }
    }
}

/// Single attempt: substitute one fake account with corrupted discriminator.
fn run_attempt<F>(
    fixture: &mut F,
    spec: &fuzz::InstructionSpec,
    idx: usize,
    wrong_disc: [u8; 8],
    expected_disc: [u8; 8],
) where
    F: HasContext + HasInstructionSet,
{
    if idx >= spec.accounts.len() {
        return;
    }

    let pubkeys: Vec<_> = spec.accounts.iter().map(|m| m.pubkey).collect();
    let saves = save_accounts(fixture.ctx(), &pubkeys);
    let pre_hash = hash_accounts(&saves);

    // Get real account bytes for cloning into fake
    let real_pubkey = spec.accounts[idx].pubkey;
    let Some((_, real_account)) = saves.iter().find(|(pk, _)| *pk == real_pubkey).cloned() else {
        restore_accounts(fixture.ctx_mut(), saves);
        return;
    };

    // Need at least 8 bytes to corrupt discriminator
    if real_account.data.len() < 8 {
        restore_accounts(fixture.ctx_mut(), saves);
        return;
    }

    //    Build fake data: clone real, overwrite first 8 bytes
    //    KEY ORTHOGONALITY: preserve owner correctness (so owner-skip
    //    is NOT triggered) — only the discriminator-check failure path
    //    can cause this attack to succeed.
    let mut fake_data = real_account.data.clone();
    fake_data[0..8].copy_from_slice(&wrong_disc);

    let fake_pubkey = Pubkey::new_unique();
    let fake_account = Account {
        owner: real_account.owner, // PRESERVE — orthogonality with owner-skip
        data: fake_data,
        lamports: real_account.lamports,
        executable: false,
        rent_epoch: real_account.rent_epoch,
    };
    if fixture
        .ctx_mut()
        .write_account(&fake_pubkey, fake_account)
        .is_err()
    {
        restore_accounts(fixture.ctx_mut(), saves);
        return;
    }

    // Substitute fake into AccountMeta list
    let mut metas = spec.accounts.clone();
    metas[idx].pubkey = fake_pubkey;

    let ix = solana_instruction::Instruction {
        program_id: spec.program_id,
        accounts: metas,
        data: spec.data_sample.clone(),
    };

    let fee_payer = fixture.fee_payer();
    let mut signer_refs: Vec<&Keypair> = vec![&*fee_payer];
    for kp in &spec.signers {
        if kp.pubkey() != fee_payer.pubkey() {
            signer_refs.push(&**kp);
        }
    }

    let result = fixture
        .ctx_mut()
        .raw_call(ix)
        .fee_payer(&*fee_payer)
        .signers(&signer_refs)
        .send();

    //  Compute post hash
    let post_hash = hash_accounts_now(fixture.ctx(), pubkeys.iter().copied());

    let succeeded = matches!(result, Ok(TxOutcome::Success { .. }));
    let state_changed = pre_hash != post_hash;

    fuzz_assert!(
        !(succeeded && state_changed),
        "[discriminator-skip:{}] ix {} succeeded with account {} \
         discriminator {:02x?} instead of expected {:02x?}; \
         real pubkey {} → fake pubkey {}",
        spec.program_id,
        spec.name,
        idx,
        wrong_disc,
        expected_disc,
        real_pubkey,
        fake_pubkey,
    );

    //  Manual restore of originals + fake-account cleanup
    restore_accounts(fixture.ctx_mut(), saves);
    cleanup_temp_account(fixture.ctx_mut(), &fake_pubkey);
}
