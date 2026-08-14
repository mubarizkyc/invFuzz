use crucible_test_context::TestContext;
use solana_account::Account;
use solana_pubkey::Pubkey;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

pub(crate) fn hash_accounts(saves: &[(Pubkey, Account)]) -> u64 {
    let mut h = DefaultHasher::new();
    for (pk, acct) in saves {
        h.write(pk.as_ref());
        h.write(&acct.data);
        h.write_u64(acct.lamports);
        h.write(acct.owner.as_ref());
        h.write_u8(u8::from(acct.executable));
        h.write_u64(acct.rent_epoch);
    }
    h.finish()
}

pub(crate) fn hash_accounts_now<I>(ctx: &TestContext, pubkeys: I) -> u64
where
    I: IntoIterator<Item = Pubkey>,
{
    let saves: Vec<_> = pubkeys
        .into_iter()
        .filter_map(|pk| ctx.get_account(&pk).ok().map(|a| (pk, a)))
        .collect();
    hash_accounts(&saves)
}

pub(crate) fn save_accounts(ctx: &TestContext, pubkeys: &[Pubkey]) -> Vec<(Pubkey, Account)> {
    pubkeys
        .iter()
        .filter_map(|pk| ctx.get_account(pk).ok().map(|a| (*pk, a)))
        .collect()
}

pub(crate) fn restore_accounts(ctx: &mut TestContext, saves: Vec<(Pubkey, Account)>) {
    for (pk, acct) in saves {
        let _ = ctx.write_account(&pk, acct);
    }
}
pub(crate) fn cleanup_temp_account(ctx: &mut TestContext, pubkey: &Pubkey) {
    let _ = ctx.write_account(
        pubkey,
        Account {
            lamports: 0,
            data: Vec::new(),
            owner: Pubkey::default(),
            executable: false,
            rent_epoch: 0,
        },
    );
}
