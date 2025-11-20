use anchor_lang::prelude::*;

#[account]
pub struct StealthAirdropVault {
    pub swappers: Vec<[u8; 32]>,
    pub unique_count: u64,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct InitializeStealthVault<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 4 + (32 * 80_000) + 8 + 1,
        seeds = [b"stealth_airdrop_vault"],
        bump
    )]
    pub vault: Account<'info, StealthAirdropVault>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AppendSwapper<'info> {
    #[account(
        mut,
        seeds = [b"stealth_airdrop_vault"],
        bump = vault.bump
    )]
    pub vault: Account<'info, StealthAirdropVault>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_stealth_vault(ctx: Context<InitializeStealthVault>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    vault.unique_count = 0;
    vault.bump = ctx.bumps.vault;
    Ok(())
}

pub fn append_swapper(ctx: Context<AppendSwapper>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    let wallet_bytes = ctx.accounts.user.key().to_bytes();

    if vault.swappers.contains(&wallet_bytes) {
        return Ok(());
    }

    if vault.unique_count >= 80_000 {
        return err!(SafePumpError::AirdropLimitExceeded);
    }

    vault.swappers.push(wallet_bytes);
    vault.unique_count += 1;

    emit!(SwapperCaptured {
        wallet: ctx.accounts.user.key(),
        total_unique: vault.unique_count
    });

    Ok(())
}

#[event]
pub struct SwapperCaptured {
    pub wallet: Pubkey,
    pub total_unique: u64,
}
