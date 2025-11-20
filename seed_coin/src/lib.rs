use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Mint, Token, TokenAccount, MintTo, Transfer, Burn},
};
use solana_program::{program::invoke, clock::Clock, sysvar::Sysvar};
use raydium_cp_swap::cpi::{accounts::CreatePool, create_pool};
use raydium_cp_swap::instruction::SwapBaseInput;

declare_id!("AymD4HzxTN2SK6UDrCcXD2uAFk4RptvQKzMT5P9GSr32");

// ─────────────────────────────────────────────────────────────────────────────
// MOTHERSHIP ID & GLOBAL TAX
// ─────────────────────────────────────────────────────────────────────────────
const SAFEPUMP_MOTHERSHIP_ID: Pubkey = Pubkey::new_from_array([
    0xA7, 0xB4, 0xA4, 0x4B, 0xB4, 0xD2, 0x0C, 0x2A, 0xB5, 0x73, 0x9C, 0xF7,
    0xD6, 0xC2, 0xD2, 0x25, 0x9B, 0xA0, 0xF0, 0x5A, 0xC2, 0x39, 0xB4, 0x49,
    0xD3, 0xA4, 0xF5, 0xD7, 0x92, 0xD3, 0xCD, 0x32,
]);

const GLOBAL_TAX: u64 = 100;
const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

// ─────────────────────────────────────────────────────────────────────────────
// CONFIGURABLE TOP-TIER MCAP (1M, 5M, 10M, 50M, 100M)
// ─────────────────────────────────────────────────────────────────────────────
const VALID_TOP_TIER_SOL: &[u64] = &[1_000_000, 5_000_000, 10_000_000, 50_000_000, 100_000_000];

// ─────────────────────────────────────────────────────────────────────────────
// FIBONACCI TIER SYSTEM
// ─────────────────────────────────────────────────────────────────────────────
const FIB_START_BPS: u64 = 1;
const MAX_SWAP_BPS_AT_TOP_TIER: u64 = 100;
const FIB_TIERS: [u64; 8] = [1, 2, 3, 5, 8, 13, 21, 34];
const FIB_MCAP_THRESHOLDS: [u64; 8] = [
    100_000, 500_000, 1_000_000, 5_000_000,
    10_000_000, 25_000_000, 50_000_000, 75_000_000,
];

// ─────────────────────────────────────────────────────────────────────────────
// LOCAL CONFIG
// ─────────────────────────────────────────────────────────────────────────────
const MIN_TOTAL_SUPPLY: u64 = 1_000_000_000_000_000;
const MAX_TOTAL_SUPPLY: u64 = 1_000_000_000_000_000_000;
const MIN_SWAP_FEE: u64 = 100;
const MAX_SWAP_FEE: u64 = 1000;
const MIN_MAX_TOKENS_BUY: u64 = 10;
const MAX_MAX_TOKENS_BUY: u64 = 100;
const MIN_MAX_HOLDINGS_SELL: u64 = 10;
const MAX_MAX_HOLDINGS_SELL: u64 = 100;
const VALID_SWAP_PERIODS: &[i64] = &[900, 1800, 3600, 14400, 28800, 86400];
const MAX_NAME_LEN: usize = 32;
const MAX_TICKER_LEN: usize = 16;
const MAX_DESCRIPTION_LEN: usize = 288;
const MAX_FRIENDS_WALLETS: usize = 4;
const MAX_ALLOCATION_PERCENT: u64 = 5100;
const MIN_VAULT_SEED_SOL: u64 = 250_000_000;
const MAX_VAULT_SEED_SOL: u64 = 10_000_000_000;
const BOND_THRESHOLD_SOL: u64 = 100_000_000_000; // 100 SOL
const AIRDROP_TRIGGER_COUNT: usize = 1000;
const AIRDROP_PER_USER_BPS: u64 = 1; // 0.001% = 1 / 100_000

// ─────────────────────────────────────────────────────────────────────────────
// EVENTS & ERRORS
// ─────────────────────────────────────────────────────────────────────────────
#[event]
pub struct HandshakeEvent {
    pub program_id: Pubkey,
    pub deployer_wallet: Pubkey,
}

#[error_code]
pub enum MemeTemplateError {
    #[msg("Invalid supply")] InvalidSupply,
    #[msg("Anti-sniper cooldown")] AntiSniperCooldown,
    #[msg("Contract not live")] NotLive,
    #[msg("Invalid argument")] InvalidArgument,
    #[msg("Invalid swap period")] InvalidSwapPeriod,
    #[msg("Invalid friends allocation")] InvalidFriendsAllocation,
    #[msg("Invalid burn percentage")] InvalidBurnPercentage,
    #[msg("Invalid LP percentage")] InvalidLpPercentage,
    #[msg("Sell lock active")] SellLockActive,
    #[msg("Invalid vault seed SOL")] InvalidVaultSeedSol,
    #[msg("Bond failed")] BondFailed,
    #[msg("Exceeds Fib-tier buy cap")] ExceedsFibBuy,
    #[msg("Exceeds Fib-tier sell cap")] ExceedsFibSell,
    #[msg("Airdrop already enabled")] AirdropAlreadyEnabled,
    #[msg("Invalid top-tier MCAP")] InvalidTopTierMcap,
    #[msg("Airdrop not triggered")] AirdropNotTriggered,
    #[msg("CPI rate limit exceeded")] CpiRateLimit,
}

// ─────────────────────────────────────────────────────────────────────────────
// ACCOUNT STRUCTS
// ─────────────────────────────────────────────────────────────────────────────
#[account]
#[derive(Default)]
pub struct MemeToken {
    pub total_supply: u64,
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub swap_fee_bps: u64,
    pub max_buy_bps: u64,
    pub max_sell_bps: u64,
    pub sell_cooldown: i64,
    pub burn_lp_percent: u8,
    pub friends: [Pubkey; MAX_FRIENDS_WALLETS],
    pub friend_amounts: [u64; MAX_FRIENDS_WALLETS],
    pub deployer_amount: u64,
    pub treasurer: Pubkey,
    pub vault_sol: u64,
    pub vault_tokens: u64,
    pub burned: u64,
    pub created_at: i64,
    pub live: bool,
    pub bonded: bool,
    pub lp_addr: Option<Pubkey>,
    pub lp_percent: u8,
    pub seed_sol: u64,
    pub swap_count: u64,
    pub airdrop_enabled: bool,
    pub airdrop_count: u64,
    pub top_tier_mcap_sol: u64,
    pub airdrop_triggered: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// HELPER: FIB TIER
// ─────────────────────────────────────────────────────────────────────────────
fn get_fib_tier(mcap_lamports: u64, top_tier_mcap_sol: u64) -> u64 {
    let top_tier_lamports = top_tier_mcap_sol * LAMPORTS_PER_SOL;

    let tier_idx = FIB_MCAP_THRESHOLDS
        .iter()
        .enumerate()
        .find(|(_, &threshold)| mcap_lamports >= threshold * LAMPORTS_PER_SOL)
        .map(|(i, _)| i)
        .unwrap_or(7);

    let fib_bps = FIB_START_BPS + FIB_TIERS[tier_idx];
    if mcap_lamports >= top_tier_lamports {
        MAX_SWAP_BPS_AT_TOP_TIER
    } else {
        fib_bps
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PROGRAM
// ─────────────────────────────────────────────────────────────────────────────
#[program]
pub mod meme_template {
    use super::*;

    pub fn initialize_meme(
        ctx: Context<InitializeMeme>,
        total_supply: u64,
        name: String,
        symbol: String,
        description: String,
        swap_fee_bps: u64,
        max_buy_bps: u64,
        max_sell_bps: u64,
        sell_cooldown: i64,
        burn_lp_percent: u8,
        friends: [Pubkey; MAX_FRIENDS_WALLETS],
        friend_amounts: [u64; MAX_FRIENDS_WALLETS],
        deployer_amount: u64,
        treasurer: Pubkey,
        lp_percent: u8,
        seed_sol: u64,
        use_airdrop: bool,
        top_tier_mcap_sol: u64,
    ) -> Result<()> {
        let clock = Clock::get()?;

        // === VALIDATION ===
        require!(total_supply >= MIN_TOTAL_SUPPLY && total_supply <= MAX_TOTAL_SUPPLY, MemeTemplateError::InvalidSupply);
        require!(name.len() <= MAX_NAME_LEN && symbol.len() <= MAX_TICKER_LEN && description.len() <= MAX_DESCRIPTION_LEN, MemeTemplateError::InvalidArgument);
        require!(swap_fee_bps >= MIN_SWAP_FEE && swap_fee_bps <= MAX_SWAP_FEE, MemeTemplateError::InvalidArgument);
        require!(max_buy_bps >= MIN_MAX_TOKENS_BUY && max_buy_bps <= MAX_MAX_TOKENS_BUY, MemeTemplateError::InvalidArgument);
        require!(max_sell_bps >= MIN_MAX_HOLDINGS_SELL && max_sell_bps <= MAX_MAX_HOLDINGS_SELL, MemeTemplateError::InvalidArgument);
        require!(VALID_SWAP_PERIODS.contains(&sell_cooldown), MemeTemplateError::InvalidSwapPeriod);
        require!(burn_lp_percent <= 50, MemeTemplateError::InvalidBurnPercentage);
        require!(lp_percent <= 100, MemeTemplateError::InvalidLpPercentage);
        require!(seed_sol >= MIN_VAULT_SEED_SOL && seed_sol <= MAX_VAULT_SEED_SOL, MemeTemplateError::InvalidVaultSeedSol);
        require!(VALID_TOP_TIER_SOL.contains(&top_tier_mcap_sol), MemeTemplateError::InvalidTopTierMcap);

        let total_alloc = deployer_amount + friend_amounts.iter().sum::<u64>();
        let alloc_pct = (total_alloc * 10_000) / total_supply;
        require!(alloc_pct <= MAX_ALLOCATION_PERCENT, MemeTemplateError::InvalidFriendsAllocation);
        require!(lp_percent == 100 - (alloc_pct / 100), MemeTemplateError::InvalidLpPercentage);

        let meme = &mut ctx.accounts.meme_token;
        *meme = MemeToken {
            total_supply, name, symbol, description, swap_fee_bps, max_buy_bps, max_sell_bps,
            sell_cooldown, burn_lp_percent, friends, friend_amounts, deployer_amount, treasurer,
            vault_sol: 0, vault_tokens: 0, burned: 0, created_at: clock.unix_timestamp,
            live: false, bonded: false, lp_addr: None, lp_percent, seed_sol, swap_count: 0,
            airdrop_enabled: false, airdrop_count: 0, top_tier_mcap_sol, airdrop_triggered: false,
        };

        if use_airdrop {
            let registry = &ctx.accounts.airdrop_registry;
            require!(registry.claimers.len() > 0, MemeTemplateError::InvalidArgument);
            require!(!meme.airdrop_enabled, MemeTemplateError::AirdropAlreadyEnabled);

            meme.airdrop_enabled = true;
            meme.airdrop_count = registry.claimers.len() as u64;
        }

        emit!(HandshakeEvent { program_id: ctx.program_id, deployer_wallet: ctx.accounts.deployer.key() });

        safe_pump::cpi::register_meme_coin(CpiContext::new(
            ctx.accounts.mothership_program.to_account_info(),
            safe_pump::cpi::accounts::RegisterMemeCoin {
                contract: ctx.accounts.mothership_contract.to_account_info(),
                meme_coin_registry: ctx.accounts.registry.to_account_info(),
                deployer: ctx.accounts.deployer.to_account_info(),
                owner: ctx.accounts.mint_authority.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                rent: ctx.accounts.rent.to_account_info(),
            },
        ), ctx.program_id)?;

        Ok(())
    }

    pub fn mint_to_vault(ctx: Context<MintToVault>) -> Result<()> {
        let meme = &mut ctx.accounts.meme_token;
        let lp_tokens = (meme.total_supply * meme.lp_percent as u64) / 100;

        token::mint_to(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo { mint: ctx.accounts.mint.to_account_info(), to: ctx.accounts.vault.to_account_info(), authority: ctx.accounts.mint_authority.to_account_info() },
            &[&[b"mint_auth", ctx.accounts.mint.key().as_ref(), &[ctx.bumps.mint_authority]]],
        ), lp_tokens)?;

        let mint_to = |to: Pubkey, amt: u64| -> Result<()> {
            let ata = get_ata(&to, &ctx.accounts.mint.key());
            invoke(&spl_associated_token_account::instruction::create_associated_token_account(
                &ctx.accounts.deployer.key(), &to, &ctx.accounts.mint.key(), &spl_token::ID,
            ), &[
                ctx.accounts.deployer.to_account_info(),
                ata.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.mint.to_account_info(),
            ])?;
            token::mint_to(CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo { mint: ctx.accounts.mint.to_account_info(), to: ata.to_account_info(), authority: ctx.accounts.mint_authority.to_account_info() },
                &[&[b"mint_auth", ctx.accounts.mint.key().as_ref(), &[ctx.bumps.mint_authority]]],
            ), amt)?;
            Ok(())
        };

        mint_to(ctx.accounts.deployer.key(), meme.deployer_amount)?;
        for (i, &friend) in meme.friends.iter().enumerate() {
            if friend != Pubkey::default() {
                mint_to(friend, meme.friend_amounts[i])?;
            }
        }

        if meme.burn_lp_percent > 0 {
            let burn = lp_tokens * meme.burn_lp_percent as u64 / 100;
            token::burn(CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn { mint: ctx.accounts.mint.to_account_info(), from: ctx.accounts.vault.to_account_info(), authority: ctx.accounts.mint_authority.to_account_info() },
            ), burn)?;
            meme.burned += burn;
        }

        token::transfer(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer { from: ctx.accounts.deployer_sol.to_account_info(), to: ctx.accounts.vault_sol.to_account_info(), authority: ctx.accounts.deployer.to_account_info() },
        ), meme.seed_sol)?;
        meme.vault_sol = meme.seed_sol;
        meme.vault_tokens = lp_tokens - meme.burned;
        meme.live = true;

        Ok(())
    }

    pub fn swap(
        ctx: Context<Swap>,
        amount_in: u64,
        is_buy: bool,
        min_out: u64,
    ) -> Result<()> {
        let meme = &mut ctx.accounts.meme_token;
        let clock = Clock::get()?;
        require!(meme.live, MemeTemplateError::NotLive);

        if clock.unix_timestamp - meme.created_at < 120 {
            return err!(MemeTemplateError::AntiSniperCooldown);
        }

        if !is_buy {
            require!(!meme.sell_lock_active(), MemeTemplateError::SellLockActive);
            require!(clock.unix_timestamp - ctx.accounts.user_state.last_sell >= meme.sell_cooldown, MemeTemplateError::InvalidArgument);
            ctx.accounts.user_state.last_sell = clock.unix_timestamp;
        }

        let mcap_lamports = if meme.vault_tokens > 0 {
            (meme.vault_sol as u128) * (meme.total_supply as u128) / (meme.vault_tokens as u128)
        } else { 0 } as u64;

        let fib_bps = get_fib_tier(mcap_lamports, meme.top_tier_mcap_sol);
        let max_buy_allowed = meme.total_supply * fib_bps / 10_000;
        let max_sell_allowed = ctx.accounts.user_token.amount * fib_bps / 10_000;

        if is_buy {
            require!(amount_in <= max_buy_allowed.min(meme.total_supply * meme.max_buy_bps / 10_000), MemeTemplateError::ExceedsFibBuy);
        } else {
            require!(amount_in <= max_sell_allowed.min(ctx.accounts.user_token.amount * meme.max_sell_bps / 10_000), MemeTemplateError::ExceedsFibSell);
        }

        let global_tax = amount_in * GLOBAL_TAX / 10_000;
        let local_tax = amount_in * meme.swap_fee_bps / 10_000;
        let lp_amount = amount_in - global_tax - local_tax;

        if meme.bonded {
            raydium_cp_swap::cpi::swap_base_in(CpiContext::new(
                ctx.accounts.raydium_program.to_account_info(),
                raydium_cp_swap::cpi::accounts::SwapBaseIn {
                    pool_state: ctx.accounts.pool_state.to_account_info(),
                    user_source_token: if is_buy { ctx.accounts.user_sol.to_account_info() } else { ctx.accounts.user_token.to_account_info() },
                    user_destination_token: if is_buy { ctx.accounts.user_token.to_account_info() } else { ctx.accounts.user_sol.to_account_info() },
                    token_0_vault: ctx.accounts.vault.to_account_info(),
                    token_1_vault: ctx.accounts.vault_sol.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                    remaining_accounts: vec![],
                },
            ), SwapBaseInput { amount: lp_amount, minimum_amount_out: min_out })?;
        } else {
            token::transfer(CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer { from: ctx.accounts.user_sol.to_account_info(), to: ctx.accounts.vault_sol.to_account_info(), authority: ctx.accounts.user.to_account_info() },
            ), lp_amount)?;
            let price = (meme.vault_sol as u128) * 1_000_000_000 / (meme.vault_tokens as u128);
            let tokens_out = (lp_amount as u128) * 1_000_000_000 / price;
            token::mint_to(CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo { mint: ctx.accounts.mint.to_account_info(), to: ctx.accounts.user_token.to_account_info(), authority: ctx.accounts.mint_authority.to_account_info() },
                &[&[b"mint_auth", ctx.accounts.mint.key().as_ref(), &[ctx.bumps.mint_authority]]],
            ), tokens_out as u64)?;
            meme.vault_sol += lp_amount;
            meme.vault_tokens -= tokens_out as u64;
        }

        token::transfer(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer { from: ctx.accounts.user_sol.to_account_info(), to: ctx.accounts.mothership_sol_vault.to_account_info(), authority: ctx.accounts.user.to_account_info() },
        ), global_tax)?;

        if local_tax > 0 {
            let treasury_ata = get_ata(&meme.treasurer, &ctx.accounts.sol_mint.key());
            invoke(&spl_associated_token_account::instruction::create_associated_token_account(
                &ctx.accounts.user.key(), &meme.treasurer, &ctx.accounts.sol_mint.key(), &spl_token::ID,
            ), &[
                ctx.accounts.user.to_account_info(),
                treasury_ata.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.sol_mint.to_account_info(),
            ])?;
            token::transfer(CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer { from: ctx.accounts.user_sol.to_account_info(), to: treasury_ata.to_account_info(), authority: ctx.accounts.user.to_account_info() },
            ), local_tax)?;
        }

        safe_pump::cpi::global_tax_swap(CpiContext::new(
            ctx.accounts.mothership_program.to_account_info(),
            safe_pump::cpi::accounts::GlobalTaxSwap {
                contract: ctx.accounts.mothership_contract.to_account_info(),
                user: ctx.accounts.user.to_account_info(),
                user_ata: ctx.accounts.user_sol.to_account_info(),
                user_safepump_ata: ctx.accounts.user_token.to_account_info(),
                vault: ctx.accounts.mothership_vault.to_account_info(),
                sol_vault: ctx.accounts.mothership_sol_vault.to_account_info(),
                lp_vault: ctx.accounts.mothership_lp.to_account_info(),
                badge_vault: ctx.accounts.mothership_badge_vault.to_account_info(),
                swap_rewards_vault: ctx.accounts.mothership_rewards_vault.to_account_info(),
                mint: ctx.accounts.mothership_mint.to_account_info(),
                owner: ctx.accounts.mint_authority.to_account_info(),
                badge_holders: ctx.accounts.badge_holders.to_account_info(),
                user_swap_data: ctx.accounts.user_state.to_account_info(),
                reward_distribution: ctx.accounts.reward_dist.to_account_info(),
                meme_coin_data: ctx.accounts.meme_token.to_account_info(),
                pool_state: ctx.accounts.pool_state.to_account_info(),
                raydium_program: ctx.accounts.raydium_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
                rent: ctx.accounts.rent.to_account_info(),
            },
        ), global_tax, is_buy, ctx.program_id)?;

        if !meme.bonded && meme.vault_sol >= BOND_THRESHOLD_SOL {
            create_pool(CpiContext::new_with_signer(
                ctx.accounts.raydium_program.to_account_info(),
                CreatePool {
                    pool_state: ctx.accounts.pool_state.to_account_info(),
                    token0_vault: ctx.accounts.vault.to_account_info(),
                    token1_vault: ctx.accounts.vault_sol.to_account_info(),
                    lp_mint: ctx.accounts.lp_mint.to_account_info(),
                    amm_config: ctx.accounts.amm_config.to_account_info(),
                    authority: ctx.accounts.mint_authority.to_account_info(),
                    observation_state: ctx.accounts.observation_state.to_account_info(),
                    create_pool_fee: ctx.accounts.create_pool_fee.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
                &[&[b"mint_auth", ctx.accounts.mint.key().as_ref(), &[ctx.bumps.mint_authority]]],
            ))?;
            meme.lp_addr = Some(ctx.accounts.pool_state.key());
            meme.bonded = true;
        }

        // === AUTO-TRIGGER AIRDROP IF 1000+ CLAIMERS ===
        if meme.airdrop_enabled && !meme.airdrop_triggered && ctx.accounts.airdrop_registry.claimers.len() >= AIRDROP_TRIGGER_COUNT {
            safe_pump::cpi::trigger_airdrop(CpiContext::new(
                ctx.accounts.mothership_program.to_account_info(),
                safe_pump::cpi::accounts::TriggerAirdrop {
                    contract: ctx.accounts.mothership_contract.to_account_info(),
                    owner: ctx.accounts.deployer.to_account_info(),
                    airdrop_registry: ctx.accounts.airdrop_registry.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    meme_token: ctx.accounts.meme_token.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
                },
            ), ctx.program_id)?;
            meme.airdrop_triggered = true;
        }

        meme.swap_count += 1;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FULL CONTEXTS — MEME TEMPLATE v4
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct InitializeMeme<'info> {
    #[account(
        init,
        payer = deployer,
        space = 8 + 8 + 4+32 + 4+16 + 4+288 + 8 + 8 + 8 + 8 + 1 + 4*32 + 4*8 + 8 + 32 + 8 + 8 + 8 + 1 + 1 + 33 + 1 + 8 + 8 + 1 + 8 + 8 + 1,
        seeds = [b"meme", mint.key().as_ref()],
        bump
    )]
    pub meme_token: Account<'info, MemeToken>,
    #[account(mut)]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub deployer: Signer<'info>,
    #[account(seeds = [b"mint_auth", mint.key().as_ref()], bump)]
    pub mint_authority: SystemAccount<'info>,
    #[account(mut)]
    pub mothership_contract: Account<'info, safe_pump::TokenContract>,
    #[account(
        init_if_needed,
        payer = deployer,
        space = 8 + 8 + 2000*64,
        seeds = [b"registry", mothership_contract.key().as_ref()],
        bump
    )]
    pub registry: Account<'info, safe_pump::MemeCoinRegistry>,
    #[account(seeds = [b"airdrop_registry"], bump)]
    pub airdrop_registry: Account<'info, safe_pump::AirdropRegistry>,
    #[account(address = SAFEPUMP_MOTHERSHIP_ID)]
    pub mothership_program: Program<'info, safe_pump::program::SafePump>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct MintToVault<'info> {
    #[account(mut)]
    pub meme_token: Account<'info, MemeToken>,
    #[account(mut)]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault_sol: Account<'info, TokenAccount>,
    #[account(mut)]
    pub deployer: Signer<'info>,
    #[account(mut)]
    pub deployer_sol: Account<'info, TokenAccount>,
    #[account(seeds = [b"mint_auth", mint.key().as_ref()], bump)]
    pub mint_authority: SystemAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub meme_token: Account<'info, MemeToken>,
    #[account(mut)]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_sol: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault_sol: Account<'info, TokenAccount>,
    #[account(mut)]
    pub mothership_sol_vault: Account<'info, TokenAccount>,
    #[account(seeds = [b"mint_auth", mint.key().as_ref()], bump)]
    pub mint_authority: SystemAccount<'info>,
    #[account(mut)]
    pub mothership_contract: Account<'info, safe_pump::TokenContract>,
    #[account(mut)]
    pub mothership_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub mothership_lp: Account<'info, TokenAccount>,
    #[account(mut)]
    pub mothership_badge_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub mothership_rewards_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub mothership_mint: Account<'info, Mint>,
    #[account(mut, seeds = [b"badge-holders", mothership_mint.key().as_ref()], bump)]
    pub badge_holders: Account<'info, safe_pump::BadgeHolders>,
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 8 + 8 + 8 + 1,
        seeds = [b"user-swap-data", user.key().as_ref(), mint.key().as_ref()],
        bump
    )]
    pub user_state: Account<'info, safe_pump::UserSwapData>,
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + (40 * 100) + 8 + 8 + 1,
        seeds = [b"reward-distribution", mothership_mint.key().as_ref()],
        bump
    )]
    pub reward_dist: Account<'info, safe_pump::RewardDistribution>,
    #[account(seeds = [b"airdrop_registry"], bump)]
    pub airdrop_registry: Account<'info, safe_pump::AirdropRegistry>,
    #[account(mut)]
    pub pool_state: AccountInfo<'info>,
    #[account(mut)]
    pub lp_mint: Account<'info, Mint>,
    #[account(mut)]
    pub amm_config: AccountInfo<'info>,
    #[account(mut)]
    pub observation_state: AccountInfo<'info>,
    #[account(mut)]
    pub create_pool_fee: AccountInfo<'info>,
    #[account(address = SAFEPUMP_MOTHERSHIP_ID)]
    pub mothership_program: Program<'info, safe_pump::program::SafePump>,
    #[account(address = raydium_cp_swap::id())]
    pub raydium_program: Program<'info, raydium_cp_swap::program::RaydiumCpSwap>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
    pub sol_mint: Account<'info, Mint>,
}

fn get_ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[owner.as_ref(), spl_token::ID.as_ref(), mint.as_ref()], &spl_associated_token_account::ID).0
}
