use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;
pub mod state;

// use instructions::{Initialize, Deposit, RequestWithdrawal, Claim};

pub use instructions::initialize::*;
pub use instructions::deposit::*;
pub use instructions::withdraw::*;
pub use instructions::claim::*;

declare_id!("9VfuUehi2JnBgWzN8kSYhyi7vTV3YCaKDmNqN6jpLL4F");

#[program]
pub mod spl_token_vault {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, fee_bps: u16) -> Result<()> {
        instructions::initialize::handler(ctx, fee_bps)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        instructions::deposit::handler(ctx, amount)
    }

    pub fn request_withdrawal(ctx: Context<RequestWithdrawal>, receipt_amount: u64) -> Result<()> {
        instructions::withdraw::handler(ctx, receipt_amount)
    }

    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        instructions::claim::handler(ctx)
    }
}