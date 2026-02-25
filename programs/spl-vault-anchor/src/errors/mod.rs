use anchor_lang::prelude::*;

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
}