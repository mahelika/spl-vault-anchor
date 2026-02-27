use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::{errors::VaultError, state::{VaultState, WithdrawalTicket}};

#[derive(Accounts)]
pub struct Claim <'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault_state", vault_state.admin.as_ref()],
        bump = vault_state.bump,
    )]
    pub vault_state: Account<'info, VaultState>,

    #[account(
        mut,
        seeds = [b"vault_token", vault_state.key().as_ref()],
        bump = vault_state.vault_token_bump,
    )] 
    pub vault_token_account: Account<'info, TokenAccount>,

    // destination for user's tokens
    #[account(
        mut,
        constraint = user_token_account.owner == user.key(),
        constraint = user_token_account.mint == vault_state.accepted_mint,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    //admin fee collection
    #[account(
        mut,
        constraint = admin_token_account.owner == vault_state.admin,
        constraint = admin_token_account.mint == vault_state.accepted_mint,
    )]
    pub admin_token_account: Account<'info, TokenAccount>,

    // withdrawal ticket - closed and rent returned to user on success
    #[account(
        mut,
        close = user, //acc closes adn returns rent to the user
        seeds = [b"withdrawal", user.key().as_ref(), vault_state.key().as_ref()],
        bump = withdrawal_ticket.bump,
        constraint = withdrawal_ticket.user == user.key(),
    )]
    pub withdrawal_ticket: Account<'info, WithdrawalTicket>,

    pub clock: Sysvar<'info, Clock>,
    pub token_program: Program<'info, Token>,
}

pub fn handle_claim(ctx: Context<Claim>) -> Result <()> {
    let ticket = &ctx.accounts.withdrawal_ticket;
    let now = ctx.accounts.clock.unix_timestamp;

    //enforce 24hr cooldown
    require!(
        now >= ticket.requested_at + WithdrawalTicket::COOLDOWN_SECONDS,
        VaultError::CooldownNotElapsed
    );

    let receipt_amount = ticket.receipt_amount;
    let fee_bps = ctx.accounts.vault_state.fee_bps as u64;

    //fee calc: checket_mul + checked_div prevents overflow
    let fee_amount = receipt_amount
        .checked_mul(fee_bps)
        .ok_or(VaultError::ArithmeticOverflow)?
        .checked_div(10_000)
        .ok_or(VaultError::ArithmeticOverflow)?;

    let user_amount = receipt_amount
        .checked_sub(fee_amount)
        .ok_or(VaultError::ArithmeticOverflow)?;

    //pda signer seeds for vault_token_account authority
    let admin_key = ctx.accounts.vault_state.admin;
    let vault_bump = ctx.accounts.vault_state.bump;
    let seeds: &[&[&[u8]]] = &[&[b"vault_state", admin_key.as_ref(), &[vault_bump]]];

    //transfer to user
    if user_amount > 0 {
        let user_transfer = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault_token_account.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.vault_state.to_account_info(),
            },
            seeds,
        );
        token::transfer(user_transfer, user_amount)?;
    }

    // fee to admin
    if fee_amount > 0{
        let fee_transfer = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault_token_account.to_account_info(),
                to: ctx.accounts.admin_token_account.to_account_info(),
                authority: ctx.accounts.vault_state.to_account_info(),
            },
            seeds,
        );
        token::transfer(fee_transfer, fee_amount)?;
    }

    //update state
    ctx.accounts.vault_state.total_deposited = ctx
        .accounts
        .vault_state
        .total_deposited
        .checked_sub(receipt_amount)
        .ok_or(VaultError::ArithmeticOverflow)?;

    msg!("Claimed {} tokens. Fee: {}. User received: {}", receipt_amount, fee_amount, user_amount);
    Ok(())
}