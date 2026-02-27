use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount, Transfer};
use crate::{errors::VaultError, state::VaultState};

#[derive(Accounts)]
pub struct Deposit<'info>{
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault_state", vault_state.admin.as_ref()],
        bump = vault_state.bump,
    )]
    pub vault_state: Account<'info, VaultState>,

    //user's token acc (source of deposit)
    #[account(
        mut,
        constraint = user_token_account.owner == user.key(),
        constraint = user_token_account.mint == vault_state.accepted_mint,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    // vault's token account (destination of deposit)
    #[account(
        mut,
        seeds = [b"vault_token", vault_state.key().as_ref()],
        bump = vault_state.vault_token_bump,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    // receipt mint 
    #[account(
        mut,
        constraint = receipt_mint.key() == vault_state.receipt_mint,
    )]
    pub receipt_mint: Account<'info, Mint>,

    // user's receipt token account 
    #[account(
        mut,
        constraint = user_receipt_account.owner == user.key(),
        constraint = user_receipt_account.mint == vault_state.receipt_mint,
    )]
    pub user_receipt_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<Deposit>, amount:u64) -> Result<()> {
    require!(!ctx.accounts.vault_state.is_paused, VaultError::VaultPaused);

    //1: transfer tokens from user -> vault (CPI) - user is the signer.
    let transfer_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(), //user signs
        },
    );
    token::transfer(transfer_ctx, amount)?;

    //2: mint receipt tokens to the user (cpi with pda signer) - vault_state pda is the mint auth and needs to sign
    let admin_key = ctx.accounts.vault_state.admin;
    let bump = ctx.accounts.vault_state.bump;
    let seeds: &[&[&[u8]]] = &[&[b"vault_state", admin_key.as_ref(), &[bump]]];

    let mint_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        MintTo {
            mint: ctx.accounts.receipt_mint.to_account_info(),
            to: ctx.accounts.user_receipt_account.to_account_info(),
            authority: ctx.accounts.vault_state.to_account_info(),
        },
        seeds,
    );
    token::mint_to(mint_ctx, amount)?; // 1:1 exchange

    //3: update state after cpis
    ctx.accounts.vault_state.total_deposited = ctx
        .accounts
        .vault_state
        .total_deposited
        .checked_add(amount) //u64 can overflow
        .ok_or(VaultError::ArithmeticOverflow)?;

    msg!("Deposited {} tokens. Total in vault: {}.", amount, ctx.accounts.vault_state.total_deposited);
    Ok(())
}