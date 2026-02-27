use anchor_lang::prelude::*;

// pda seeds: [b"vault_state", admin.key().s_ref()]

#[account]
#[derive(Default)]
pub struct VaultState {
    pub admin: Pubkey,
    pub accepted_mint: Pubkey,
    pub receipt_mint: Pubkey,
    pub total_deposited: u64,
    pub fee_bps: u16,
    pub is_paused: bool,
    pub bump: u8,
    pub vault_token_bump: u8,
}

impl VaultState {
    pub const LEN: usize = 8 + 32 + 32 +32 +8 +2 + 1 + 1 +1;
}

#[account]
pub struct WithdrawalTicket {
    pub user: Pubkey,
    pub receipt_amount: u64,
    pub requested_at: i64,
    pub bump: u8,
}

impl WithdrawalTicket {
    pub const LEN: usize = 8 + 32 + 8 + 8 + 1;
    pub const COOLDOWN_SECONDS: i64 = 86_400;
}