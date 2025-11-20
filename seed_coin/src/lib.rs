//new memetemplate seed coin child v5
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount, MintTo, Transfer},
};
use solana_program::clock::Clock;
use raydium_cp_swap::cpi::{accounts::{CreatePool, SwapBaseIn}, create_pool, swap_base_in};
use raydium_cp_swap::instruction::SwapBaseInput;

// BLS12-381
use blstrs::{G1Affine, G1Projective, G2Projective};
use subtle::CtOption;

// INJECTED AT COMPILE TIME
declare_id!(CymD4HzxTN2SK6UDrCcXD2uAFk4RptvQKzMT5P9GSr32(concat!(env!("OUT_DIR"), "/generated_program_ids.rs"));

pub const MOTHERSHIP_PROGRAM_ID: Pubkey = MOTHERSHIP_PUBKEY;
pub const MEME_MINT_SUFFIX: &str = "SPMP";

// ─────────────────────────────────────────────────────────────────────────────
// CONSTANTS — 100% MATCH FRONTEND + MOTHERSHIP
// ─────────────────────────────────────────────────────────────────────────────
const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
const BOND_THRESHOLD_SOL: u64 = 100_000_000_000;
const VALID_TOP_TIER_MCAP_SOL: &[u64] = &[1_000_000, 5_000_000, 10_000_000, 50_000_000, 100_000_000];
const VALID_COOLDOWNS: &[i64] = &[900, 1800, 3600, 14_400, 28_800, 86_400];

// Fibonacci Tiers
const FIB_VELOCITY_SOL: [u64; 8] = [1, 3, 7, 15, 30, 70, 150, 300];
const FIB_MCAP_THRESHOLDS_LAMPORTS: [u64; 8] = [
    1_000_000 * LAMPORTS_PER_SOL, 3_000_000 * LAMPORTS_PER_SOL,
    7_000_000 * LAMPORTS_PER_SOL, 15_000_000 * LAMPORTS_PER_SOL,
    30_000_000 * LAMPORTS_PER_SOL, 70_000_000 * LAMPORTS_PER_SOL,
    150_000_000 * LAMPORTS_PER_SOL, 300_000_000 * LAMPORTS_PER_SOL,
];
const FIB_START_BPS: u64 = 1;
const MAX_SWAP_BPS_AT_TOP_TIER: u64 = 100;
const FIB_TIERS: [u64; 8] = [1, 2, 3, 5, 8, 13, 21, 34];
const FIB_MCAP_THRESHOLDS: [u64; 8] = [
    100_000, 500_000, 1_000_000, 5_000_000,
    10_000_000, 25_000_000, 50_000_000, 75_000_000,
];

// ─────────────────────────────────────────────────────────────────────────────
// ACCOUNT STRUCTS
// ─────────────────────────────────────────────────────────────────────────────
#[account]
pub struct TokenContract {
    pub is_initialized: bool,
    pub total_supply: u64,
    pub treasury_wallet: Pubkey,
    pub burn_percentage: u8,
    pub lp_percentage: u8,
    pub friends_wallets: [Pubkey; 4],
    pub friends_amounts: [u64; 4],
    pub deployer_amount: u64,
    pub swap_fee_bps: u64,
    pub max_buy_bps: u64,
    pub max_sell_bps: u64,
    pub sell_cooldown: i64,
    pub top_tier_mcap_sol: u64,
    pub vault_sol_balance: u64,
    pub vault_token_balance: u64,
    pub burned_tokens: u64,
    pub bond_timestamp: i64,
    pub bonded: bool,
    pub airdrop_enabled: bool,
    pub airdrop_triggered: bool,
    pub bump: u8,
}

#[account]
pub struct UserSwapData {
    pub last_swap_timestamp: i64,
    pub last_cpi_timestamp: i64,
    pub cpi_count: u64,
    pub bump: u8,
    pub vault: Pubkey,
}

#[account]
pub struct Vault {
    pub bump: u8,
    pub nonce: u64,
    pub last_signer: Pubkey,
}

// ─────────────────────────────────────────────────────────────────────────────
// ERRORS
// ─────────────────────────────────────────────────────────────────────────────
#[error_code]
pub enum ChildError {
    #[msg("Invalid mint suffix")] InvalidMintSuffix,
    #[msg("Already initialized")] AlreadyInitialized,
    #[msg("Invalid supply")] InvalidSupply,
    #[msg("Invalid burn percentage")] InvalidBurnPercentage,
    #[msg("Invalid LP percentage")] InvalidLpPercentage,
    #[msg("Invalid friends allocation")] InvalidFriendsAllocation,
    #[msg("Anti-sniper cooldown")] AntiSniperCooldown,
    #[msg("Sell cooldown not met")] SellCooldownNotMet,
    #[msg("Exceeds Fib-tier buy cap")] ExceedsFibBuy,
    #[msg("Exceeds Fib-tier sell cap")] ExceedsFibSell,
    #[msg("Exceeds velocity limit")] ExceedsVelocityLimit,
    #[msg("Invalid BLS signature")] InvalidBlsSignature,
    #[msg("Invalid nonce")] InvalidNonce,
    #[msg("Vault not registered")] VaultNotRegistered,
    #[msg("Invalid top-tier MCAP")] InvalidTopTierMcap,
    #[msg("Invalid cooldown")] InvalidCooldown,
    #[msg("Math overflow")] MathError,
}

// ─────────────────────────────────────────────────────────────────────────────
// BLS VERIFICATION — v5 HASH
// ─────────────────────────────────────────────────────────────────────────────
fn verify_bls_sig(sig: [u8; 96], pk: [u8; 48], msg: &[u8]) -> bool {
    let sig = match G2Projective::from_compressed(&sig) { CtOption::Some(s) => s, _ => return false };
    let pk = match G1Affine::from_compressed(&pk) { CtOption::Some(p) => p, _ => return false };
    let h = G2Projective::hash_to_curve(msg, b"SAFE-PUMP-V5", &[]).to_affine();
    let g1 = G1Projective::generator().to_affine();
    blstrs::pairing(&g1, &sig.to_affine()) == blstrs::pairing(&pk.into(), &h)
}

macro_rules! require_spmp_suffix {
    ($mint:expr) => {{
        let s = $mint.key().to_string();
        require!(s.ends_with(MEME_MINT_SUFFIX), ChildError::InvalidMintSuffix);
    }};
}

// ─────────────────────────────────────────────────────────────────────────────
// PROGRAM
// ─────────────────────────────────────────────────────────────────────────────
#[program]
pub mod safe_pump_child {
    use super::*;

    pub fn initialize_contract(
        ctx: Context<InitializeContract>,
        total_supply: u64,
        treasury_wallet: Pubkey,
        burn_percentage: u8,
        lp_percentage: u8,
        friends_wallets: Vec<Pubkey>,
        friends_amounts: Vec<u64>,
        deployer_amount: u64,
        swap_fee_bps: u64,
        max_buy_bps: u64,
        max_sell_bps: u64,
        sell_cooldown: i64,
        top_tier_mcap_sol: u64,
        airdrop_enabled: bool,
    ) -> Result<()> {
        require_spmp_suffix!(ctx.accounts.mint);
        require!(total_supply >= 1_000_000_000_000_000, ChildError::InvalidSupply);
        require!(VALID_TOP_TIER_MCAP_SOL.contains(&top_tier_mcap_sol), ChildError::InvalidTopTierMcap);
        require!(VALID_COOLDOWNS.contains(&sell_cooldown), ChildError::InvalidCooldown);

        let total_alloc = deployer_amount + friends_amounts.iter().sum::<u64>();
        let alloc_pct = (total_alloc * 10_000) / total_supply;
        require!(alloc_pct <= 5100, ChildError::InvalidFriendsAllocation);

        let contract = &mut ctx.accounts.contract;
        contract.is_initialized = true;
        contract.total_supply = total_supply;
        contract.treasury_wallet = treasury_wallet;
        contract.burn_percentage = burn_percentage;
        contract.lp_percentage = lp_percentage;
        contract.deployer_amount = deployer_amount;
        contract.swap_fee_bps = swap_fee_bps;
        contract.max_buy_bps = max_buy_bps;
        contract.max_sell_bps = max_sell_bps;
        contract.sell_cooldown = sell_cooldown;
        contract.top_tier_mcap_sol = top_tier_mcap_sol;
        contract.airdrop_enabled = airdrop_enabled;
        contract.bond_timestamp = Clock::get()?.unix_timestamp;
        contract.bump = ctx.bumps.contract;

        for (i, (w, a)) in friends_wallets.iter().zip(friends_amounts.iter()).enumerate() {
            contract.friends_wallets[i] = *w;
            contract.friends_amounts[i] = *a;
        }

        safe_pump::cpi::handshake(
            CpiContext::new(
                ctx.accounts.mothership_program.to_account_info(),
                safe_pump::cpi::accounts::Handshake {
                    deployer: ctx.accounts.deployer.to_account_info(),
                    meme_mint: ctx.accounts.mint.to_account_info(),
                    registry: ctx.accounts.registry.to_account_info(),
                },
            ),
            ctx.program_id,
        )?;

        Ok(())
    }

    pub fn swap(
        ctx: Context<ChildSwap>,
        amount_in: u64,
        is_buy: bool,
        minimum_amount_out: u64,
        bls_sig: [u8; 96],
        bls_pk: [u8; 48],
        nonce: u64,
    ) -> Result<()> {
        require_spmp_suffix!(ctx.accounts.mint);

        let contract = &mut ctx.accounts.contract;
        let clock = Clock::get()?;

        // Anti-sniper
        if contract.vault_sol_balance == 0 && is_buy {
            require!(clock.unix_timestamp - contract.bond_timestamp >= 120, ChildError::AntiSniperCooldown);
        }

        // Sell cooldown
        require!(
            clock.unix_timestamp - ctx.accounts.user_state.last_swap_timestamp >= contract.sell_cooldown || is_buy,
            ChildError::SellCooldownNotMet
        );

        // ZK Vault + BLS
        require_keys_eq!(ctx.accounts.vault.key(), ctx.accounts.user_state.vault, ChildError::VaultNotRegistered);
        require!(ctx.accounts.vault.nonce == nonce, ChildError::InvalidNonce);

        let mut msg = Vec::new();
        msg.extend_from_slice(&amount_in.to_le_bytes());
        msg.push(if is_buy { 1 } else { 0 });
        msg.extend_from_slice(&minimum_amount_out.to_le_bytes());
        msg.extend_from_slice(&nonce.to_le_bytes());
        msg.extend_from_slice(ctx.accounts.user.key().as_ref());
        require!(verify_bls_sig(bls_sig, bls_pk, &msg), ChildError::InvalidBlsSignature);
        

      // STEALTH VAULT CAPTURE
        if ctx.accounts.contract.airdrop_enabled {
            let vault = ctx.remaining_accounts.get(0)
                .ok_or(error!(SafePumpError::MathError))?;
            safe_pump::cpi::append_swapper(CpiContext::new(
                ctx.accounts.mothership_program.to_account_info(),
                safe_pump::cpi::accounts::AppendSwapper {
                    vault: vault.to_account_info(),
                    user: ctx.accounts.user.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                },
            ))?;
        }





        // Global 2.5% tax
        safe_pump::cpi::global_tax_swap(
            CpiContext::new(
                ctx.accounts.mothership_program.to_account_info(),
                safe_pump::cpi::accounts::GlobalTaxSwap {
                    global_state: ctx.accounts.global_state.to_account_info(),
                    user: ctx.accounts.user.to_account_info(),
                    user_sol: ctx.accounts.user_sol.to_account_info(),
                    vault: ctx.accounts.vault.to_account_info(),
                    expected_vault: ctx.accounts.expected_vault.to_account_info(),
                    lp_vault: ctx.accounts.lp_vault.to_account_info(),
                    treasury_vault: ctx.accounts.treasury_vault.to_account_info(),
                    rewards: ctx.accounts.rewards.to_account_info(),
                    badge_holders: ctx.accounts.badge_holders.to_account_info(),
                    velocity: ctx.accounts.velocity.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                },
            ),
            amount_in, is_buy, minimum_amount_out, bls_sig, bls_pk, nonce,
        )?;

        let net_amount = amount_in * 9750 / 10_000;

        // MCAP calculation
        let mcap_lamports = if contract.vault_token_balance > 0 {
            (contract.vault_sol_balance as u128 * contract.total_supply as u128 / contract.vault_token_balance as u128) as u64
        } else { 0 };

        // Velocity tier
        let tier_idx = FIB_MCAP_THRESHOLDS_LAMPORTS.iter()
            .position(|&t| mcap_lamports >= t)
            .unwrap_or(7);

        if is_buy {
            let limit = FIB_VELOCITY_SOL[tier_idx] * LAMPORTS_PER_SOL;
            let new_total = ctx.accounts.velocity.total_bought_sol.checked_add(net_amount).ok_or(ChildError::MathError)?;
            require!(new_total <= limit, ChildError::ExceedsVelocityLimit);
            ctx.accounts.velocity.total_bought_sol = new_total;
            if ctx.accounts.velocity.block_slot != clock.slot {
                ctx.accounts.velocity.block_slot = clock.slot;
            }
        
        }

        // Dynamic Fib caps
        let fib_bps = if mcap_lamports >= contract.top_tier_mcap_sol * LAMPORTS_PER_SOL {
            MAX_SWAP_BPS_AT_TOP_TIER
        } else {
            let tier = FIB_MCAP_THRESHOLDS.iter().enumerate()
                .find(|(_, &t)| mcap_lamports >= t * LAMPORTS_PER_SOL)
                .map(|(i, _)| i)
                .unwrap_or(7);
            FIB_START_BPS + FIB_TIERS[tier]
        };

        let max_buy_allowed = contract.total_supply * fib_bps / 10_000;
        let max_sell_allowed = ctx.accounts.user_token.amount * fib_bps / 10_000;

        if is_buy {
            require!(net_amount <= max_buy_allowed.min(contract.total_supply * contract.max_buy_bps / 10_000), ChildError::ExceedsFibBuy);
        } else {
            require!(amount_in <= max_sell_allowed.min(ctx.accounts.user_token.amount * contract.max_sell_bps / 10_000), ChildError::ExceedsFibSell);
        }

        // Execute swap
        if contract.bonded {
            swap_base_in(CpiContext::new(
                ctx.accounts.raydium_program.to_account_info(),
                SwapBaseIn {
                    pool_state: ctx.accounts.pool_state.to_account_info(),
                    user_source_token: if is_buy { ctx.accounts.user_sol.to_account_info() } else { ctx.accounts.user_token.to_account_info() },
                    user_destination_token: if is_buy { ctx.accounts.user_token.to_account_info() } else { ctx.accounts.user_sol.to_account_info() },
                    token_0_vault: ctx.accounts.token_vault.to_account_info(),
                    token_1_vault: ctx.accounts.sol_vault.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                    remaining_accounts: vec![],
                },
            ), SwapBaseInput { amount_in: if is_buy { net_amount } else { amount_in }, minimum_amount_out })?;
        } else if is_buy {
            token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), Transfer {
                from: ctx.accounts.user_sol.to_account_info(),
                to: ctx.accounts.sol_vault_pre.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            }), net_amount)?;

            let tokens_out = (net_amount as u128)
                .checked_mul(contract.vault_token_balance as u128)
                .ok_or(ChildError::MathError)?
                .checked_div(contract.vault_sol_balance.max(1) as u128)
                .ok_or(ChildError::MathError)? as u64;

            token::mint_to(CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo { mint: ctx.accounts.mint.to_account_info(), to: ctx.accounts.user_token.to_account_info(), authority: ctx.accounts.contract.to_account_info() },
                &[&[b"contract", ctx.accounts.deployer.key().as_ref(), &[contract.bump]]]
            ), tokens_out)?;

            contract.vault_sol_balance += net_amount;
            contract.vault_token_balance -= tokens_out;
        }

        // Auto-bond
        if !contract.bonded && contract.vault_sol_balance >= BOND_THRESHOLD_SOL {
            create_pool(CpiContext::new_with_signer(
                ctx.accounts.raydium_program.to_account_info(),
                CreatePool {
                    pool_state: ctx.accounts.pool_state.to_account_info(),
                    token_0_vault: ctx.accounts.token_vault_pre.to_account_info(),
                    token_1_vault: ctx.accounts.sol_vault_pre.to_account_info(),
                    lp_mint: ctx.accounts.lp_mint.to_account_info(),
                    amm_config: ctx.accounts.amm_config.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                    observation_state: ctx.accounts.observation_state.to_account_info(),
                    create_pool_fee: ctx.accounts.create_pool_fee.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
                &[&[b"contract", ctx.accounts.deployer.key().as_ref(), &[contract.bump]]],
            ))?;
            contract.bonded = true;
        }

        ctx.accounts.user_state.last_swap_timestamp = clock.unix_timestamp;
        ctx.accounts.vault.nonce = nonce + 1;

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CONTEXTS
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct InitializeContract<'info> {
    #[account(init, payer = deployer, space = 500, seeds = [b"contract", deployer.key().as_ref()], bump)]
    pub contract: Account<'info, TokenContract>,
    #[account(mut)] pub deployer: Signer<'info>,
    #[account(mut)] pub mint: Account<'info, Mint>,
    #[account(address = MOTHERSHIP_PROGRAM_ID)] pub mothership_program: Program<'info, safe_pump::program::SafePump>,
    #[account(mut)] pub registry: Account<'info, safe_pump::MemeCoinRegistry>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct ChildSwap<'info> {
    #[account(mut)] pub user: Signer<'info>,
    #[account(mut)] pub user_sol: Account<'info, TokenAccount>,
    #[account(mut)] pub user_token: Account<'info, TokenAccount>,
    #[account(mut)] pub mint: Account<'info, Mint>,
    #[account(mut, seeds = [b"contract", contract.deployer.key().as_ref()], bump = contract.bump)]
    pub contract: Account<'info, TokenContract>,

    // Pre-bond
    #[account(mut)] pub token_vault_pre: Account<'info, TokenAccount>,
    #[account(mut)] pub sol_vault_pre: Account<'info, TokenAccount>,

    // Post-bond
    #[account(mut)] pub pool_state: AccountInfo<'info>,
    #[account(mut)] pub token_vault: Account<'info, TokenAccount>,
    #[account(mut)] pub sol_vault: Account<'info, TokenAccount>,
    #[account(mut)] pub lp_mint: Account<'info, Mint>,
    #[account(mut)] pub amm_config: AccountInfo<'info>,
    #[account(mut)] pub authority: AccountInfo<'info>,
    #[account(mut)] pub observation_state: AccountInfo<'info>,
    #[account(mut)] pub create_pool_fee: AccountInfo<'info>,

    #[account(address = raydium_cp_swap::id())] pub raydium_program: Program<'info, raydium_cp_swap::program::RaydiumCpSwap>,

    // Mothership
    #[account(address = MOTHERSHIP_PROGRAM_ID)] pub mothership_program: Program<'info, safe_pump::program::SafePump>,
    #[account(mut)] pub global_state: Account<'info, safe_pump::GlobalState>,
    #[account(mut)] pub lp_vault: Account<'info, TokenAccount>,
    #[account(mut)] pub treasury_vault: Account<'info, TokenAccount>,
    #[account(mut)] pub rewards: Account<'info, safe_pump::RewardDistribution>,
    #[account(mut)] pub badge_holders: Account<'info, safe_pump::BadgeHolders>,

    // ZK Vault
    #[account(mut, seeds = [b"zk_vault", user.key().as_ref()], bump)] pub vault: Account<'info, Vault>,
    #[account(seeds = [b"zk_vault", user.key().as_ref()], bump)] pub expected_vault: AccountInfo<'info>,

    // Velocity
    #[account(init_if_needed, payer = user, space = 8 + 8 + 8 + 1, seeds = [b"velocity", &clock.slot.to_le_bytes()], bump)]
    pub velocity: Account<'info, safe_pump::BlockSwapState>,

    #[account(mut)] pub user_state: Account<'info, UserSwapData>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
    pub rent: Sysvar<'info, Rent>,
}

pub use safe_pump::{GlobalState, BlockSwapState, MemeCoinRegistry, RewardDistribution, BadgeHolders};
