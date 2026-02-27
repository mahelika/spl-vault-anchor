use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use crate::state::VaultState;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    pub accepted_mint: Account<'info, Mint>,

    // receipt token mint
    #[account(
        init,
        payer = admin,
        mint::decimals = accepted_mint.decimals,
        mint::authority = vault_state, //only vault can mint/burn
    )]
    pub receipt_mint: Account<'info, Mint>,

    // global vault state pda
    #[account(
        init,
        payer = admin,
        space = VaultState::LEN,
        seeds = [b"vault_state", admin.key().as_ref()],
        bump
    )]
    pub vault_state: Account<'info, VaultState>,

    //token acc that holds deposited tokens - pda owned by vault_state
    #[account(
        init,
        payer = admin,
        token::mint = accepted_mint,
        token::authority = vault_state, //vault_state pda controls this acc
        seeds = [b"vault_token", vault_state.key().as_ref()],
        bump
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handle_initialize(ctx: Context<Initialize>, fee_bps: u16) -> Result<()> {
    let vault_state = &mut ctx.accounts.vault_state;

    vault_state.admin = ctx.accounts.admin.key();
    vault_state.accepted_mint = ctx.accounts.accepted_mint.key();
    vault_state.receipt_mint = ctx.accounts.receipt_mint.key();
    vault_state.total_deposited = 0;
    vault_state.fee_bps = fee_bps;
    vault_state.is_paused = false;
    vault_state.bump = ctx.bumps.vault_state;
    vault_state.vault_token_bump = ctx.bumps.vault_token_account;

    msg!("Vault initialised. Admin: {}", vault_state.admin);
    Ok(())
}

