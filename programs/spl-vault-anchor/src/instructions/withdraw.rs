use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount};
use crate::{errors::VaultError, state::{VaultState, WithdrawalTicket}};

#[derive(Accounts)]
pub struct RequestWithdrawal<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault_state", vault_state.admin.as_ref()],
        bump = vault_state.bump,
    )]
    pub vault_state: Account<'info, VaultState>,

    //receipt mint - to burn on withdrawal
    #[account(
        mut,
        constraint = receipt_mint.key() == vault_state.receipt_mint,
    )]
    pub receipt_mint: Account<'info, Mint>,

    //user's recipt token acc - source of burn
    #[account(
        mut,
        constraint = user_receipt_account.owner == user.key(),
        constraint = user_receipt_account.mint == vault_state.receipt_mint,
    )]
    pub user_receipt_account: Account<'info, TokenAccount>,

    // withdrawal ticket creation
    #[account(
        init,
        payer = user,
        space = WithdrawalTicket::LEN,
        seeds = [b"withdrawal", user.key().as_ref(), vault_state.key().as_ref()],
        bump,
    )]
    pub withdrawal_ticket: Account<'info, WithdrawalTicket>,

    pub clock: Sysvar<'info, Clock>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handle_withdraw(ctx: Context<RequestWithdrawal>, receipt_amount: u64) -> Result<()> {
    require!(!ctx.accounts.vault_state.is_paused, VaultError::VaultPaused);
    require!(
        ctx.accounts.user_receipt_account.amount >= receipt_amount,
        VaultError::InsufficientBalance
    );

    //burn receit tokens
    // let admin_key = ctx.accounts.vault_state.admin;
    // let bump = ctx.accounts.vault_state.bump;
    // let seeds: &[&[&[u8]]] = &[&[b"vault_state", admin_key.as_ref(), &[bump]]];

    let burn_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        Burn {
            mint: ctx.accounts.receipt_mint.to_account_info(),
            from: ctx.accounts.user_receipt_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(), //??? burn authority = token account owner OR delegate, fixed it.
        },
    );
    token::burn(burn_ctx, receipt_amount)?;

    //record the ticket
    let ticket = &mut ctx.accounts.withdrawal_ticket;
    ticket.user = ctx.accounts.user.key();
    ticket.receipt_amount = receipt_amount;
    ticket.requested_at = ctx.accounts.clock.unix_timestamp;
    ticket.bump = ctx.bumps.withdrawal_ticket;

    msg!(
        "Withdrawal requested. {} receipt tokens burned. Claimable after: {}",
        receipt_amount,
        ticket.requested_at + WithdrawalTicket::COOLDOWN_SECONDS
    );
    Ok(())
}


