#![allow(unexpected_cfgs)]
use anchor_lang::prelude::program::invoke;
use anchor_lang::prelude::*;

declare_id!("Esrcw11111111111111111111111111111111111111");

#[program]
pub mod escrow_program {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        beneficiary: Pubkey,
        unlock_slot: u64,
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        vault.depositor = ctx.accounts.depositor.key();
        vault.beneficiary = beneficiary;
        vault.unlock_slot = unlock_slot;
        vault.amount = 0;
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        require!(amount > 0, EscrowError::InvalidAmount);
        invoke(
            &system_instruction::transfer(
                &ctx.accounts.depositor.key(),
                &ctx.accounts.vault.key(),
                amount,
            ),
            &[
                ctx.accounts.depositor.to_account_info(),
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
        let vault = &mut ctx.accounts.vault;
        vault.amount = vault
            .amount
            .checked_add(amount)
            .ok_or(EscrowError::Overflow)?;
        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        let vault_lamports = **ctx.accounts.vault.try_borrow_lamports()?;
        if amount == 0 || amount > vault_lamports {
            return err!(EscrowError::InvalidAmount);
        }
        **ctx.accounts.vault.try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.depositor.try_borrow_mut_lamports()? += amount;
        Ok(())
    }

    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        let clock = Clock::get()?;
        let vault = &mut ctx.accounts.vault;
        require!(clock.slot >= vault.unlock_slot, EscrowError::StillLocked);
        let amount = vault.amount;
        require!(amount > 0, EscrowError::EmptyVault);

        **vault.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx
            .accounts
            .beneficiary
            .to_account_info()
            .try_borrow_mut_lamports()? += amount;
        vault.amount = 0;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = depositor,
        space = 8 + Vault::INIT_SPACE,
        seeds = [b"vault", depositor.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub depositor: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(
        mut,
        seeds = [b"vault", depositor.key().as_ref()],
        bump,
        has_one = depositor,
    )]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub depositor: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub vault: UncheckedAccount<'info>,
    #[account(mut)]
    pub depositor: Signer<'info>,
}

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(
        mut,
        seeds = [b"vault", depositor.key().as_ref()],
        bump,
        has_one = beneficiary,
    )]
    pub vault: Account<'info, Vault>,
    /// CHECK: only used as a PDA seed. The seeds constraint ties this to the unique vault.
    pub depositor: UncheckedAccount<'info>,
    #[account(mut)]
    pub beneficiary: Signer<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct Vault {
    pub depositor: Pubkey,
    pub beneficiary: Pubkey,
    pub unlock_slot: u64,
    pub amount: u64,
}

#[error_code]
pub enum EscrowError {
    InvalidAmount,
    Overflow,
    AlreadyUnlocked,
    StillLocked,
    EmptyVault,
}
