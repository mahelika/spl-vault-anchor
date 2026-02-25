use anchor_lang::prelude::*;

declare_id!("9VfuUehi2JnBgWzN8kSYhyi7vTV3YCaKDmNqN6jpLL4F");

#[program]
pub mod spl_vault_anchor {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
