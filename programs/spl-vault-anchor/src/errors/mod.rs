use anchor_lang::prelude::*;

// anchor error codes start at offset 6000, so 6000 -> CooldownNotElapsed, 6001-> NoPendingWithdrawal and so on..
#[error_code]
pub enum VaultError {
    #[msg("Cooldown period has not elapsed. Wait 24hrs after requesting withdrawal.")]
    CooldownNotElapsed,

    #[msg("No pending withdrawal exists for this user.")]
    NoPendingWithdrawal,

    #[msg("Withdrawal amount exceeds deposited balance.")]
    InsufficientBalance,

    #[msg("Arithmetic overflow in vault calculation.")]
    ArithmeticOverflow,

    #[msg("Vault is paused by admin.")]
    VaultPaused, //in production this should be multisig, single admin key is centralisation risk.

    #[msg("Unauthorized access.")]
    Unauthorized,   
}