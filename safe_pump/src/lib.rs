// new mothership v5
// ZERO HARD-CODED ADDRESSES | ALL IDS INJECTED AT COMPILE TIME

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::{self, AssociatedToken},
    token::{self, Mint, Token, TokenAccount, Transfer, Burn, MintTo},
};
use anchor_lang::solana_program::{program::invoke, clock::Clock, sysvar::Sysvar};
use raydium_cp_swap::cpi::{accounts::{CreatePool, SwapBaseIn}, create_pool};
use raydium_cp_swap::instruction::SwapBaseInput;

// BLS12-381
use blstrs::{G1Affine, G1Projective, G2Projective};
use subtle::CtOption;

// PROGRAM ID — INJECTED VIA build.rs
declare_id!(CymD4HzxTN2SK6UDrCcXD2uAFk4RptvQKzMT5P9GSr32(concat!(env!("OUT_DIR"), "/generated_program_ids.rs"));

pub const MOTHERSHIP_PROGRAM_ID_PUBKEY: Pubkey = MOTHERSHIP_PUBKEY;

// ─────────────────────────────────────────────────────────────────────────────
// CONSTANTS & CONFIG
// ─────────────────────────────────────────────────────────────────────────────
const GLOBAL_TAX_BPS: u64 = 250;
const GLOBAL_LP_TAX_BPS: u64 = 100;        // 1.00%
const SWAPPER_REWARD_TAX_BPS: u64 = 80;   // 0.80%
const BADGE_REWARD_TAX_BPS: u64 = 20;     // 0.20%
const TREASURY_TAX_BPS: u64 = 50;         // 0.50%

const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

const FIB_VELOCITY_SOL: [u64; 8] = [1, 3, 7, 15, 30, 70, 150, 300];
const FIB_MCAP_THRESHOLDS_LAMPORTS: [u64; 8] = [
    1_000_000 * LAMPORTS_PER_SOL, 3_000_000 * LAMPORTS_PER_SOL,
    7_000_000 * LAMPORTS_PER_SOL, 15_000_000 * LAMPORTS_PER_SOL,
    30_000_000 * LAMPORTS_PER_SOL, 70_000_000 * LAMPORTS_PER_SOL,
    150_000_000 * LAMPORTS_PER_SOL, 300_000_000 * LAMPORTS_PER_SOL,
];

const MAX_BADGE_HOLDERS: usize = 1000;
const BUY_SWAPS_FOR_BADGE: u64 = 1000;
const REWARD_DISTRIBUTION_PERIOD: i64 = 86_400;
const ANTI_SNIPER_COOLDOWN: i64 = 120;
const SWAP_COOLDOWN: i64 = 86_400;

const MAX_SUPPLY: u64 = 1_000_000_000_000_000_000;
const MAX_FRIENDS_WALLETS: usize = 4;
const MAX_ALLOCATION_PERCENT: u64 = 5100;
const POOL_SOL_AMOUNT: u64 = 1_000_000_000;
const INITIAL_VAULT_AMOUNT: u64 = 100_000_000_000;
const MAX_AIRDROP_CLAIMERS: usize = 10_000;
const AIRDROP_TRIGGER_COUNT: usize = 1000;

// SEEDS
const VAULT_SEED: &[u8] = b"zk_vault";
const MEME_REGISTRY_SEED: &[u8] = b"meme_registry";
const BADGE_EDITION_SEED: &[u8] = b"badge_master_edition";
const MEME_MINT_SUFFIX: &str = "SPMP";

// ─────────────────────────────────────────────────────────────────────────────
// ACCOUNT STRUCTS
// ─────────────────────────────────────────────────────────────────────────────
#[account]
pub struct GlobalState {
    pub is_initialized: bool,
    pub treasury_wallet: Pubkey,
    pub total_swapped: u64,
    pub swap_count: u64,
    pub bond_timestamp: i64,
    pub bump: u8,
}

#[account]
pub struct Vault {
    pub bump: u8,
    pub nonce: u64,
    pub last_signer: Pubkey,
}

#[account]
pub struct BadgeHolders {
    pub holders: [Pubkey; MAX_BADGE_HOLDERS],
    pub buy_swap_count: [(Pubkey, u64); MAX_BADGE_HOLDERS],
    pub holder_count: u64,
    pub bump: u8,
}

#[account]
pub struct RewardDistribution {
    pub swapper_rewards: [(Pubkey, u64); MAX_BADGE_HOLDERS],
    pub badge_rewards: u64,
    pub swap_count: u64,
    pub last_distribution_timestamp: i64,
    pub bump: u8,
}

#[account]
pub struct MemeCoinRegistry {
    pub entries: Vec<(Pubkey, Pubkey)>, // (meme_mint, child_program_id)
    pub bump: u8,
}

#[account]
pub struct BlockSwapState {
    pub total_bought_sol: u64,
    pub block_slot: u64,
    pub bump: u8,
}

#[account]
pub struct AirdropRegistry {
    pub claimers: Vec<Pubkey>,
    pub bump: u8,
}

#[account]
pub struct AirdropClaimRecord {
    pub claimed: bool,
    pub bump: u8,
}

#[account]
#[derive(Copy)]
pub struct TokenContract {
    pub is_initialized: bool,
    pub total_supply: u64,
    pub treasury_wallet: Pubkey,
    pub swap_count: u64,
    pub total_swapped: u64,
    pub vault_sol_balance: u64,
    pub vault_token_balance: u64,
    pub burned_tokens: u64,
    pub burn_percentage: u8,
    pub bond_timestamp: i64,
    pub buy_cap_percentage: u64,
    pub liquidity_threshold_index: u8,
    pub friends_wallets: [Pubkey; MAX_FRIENDS_WALLETS],
    pub friends_amounts: [u64; MAX_FRIENDS_WALLETS],
    pub deployer_amount: u64,
    pub bump: u8,
    pub badge_mint: Option<Pubkey>,
    pub badge_master_edition: Option<Pubkey>,
}

#[account]
pub struct UserSwapData {
    pub last_swap_timestamp: i64,
    pub last_cpi_timestamp: i64,
    pub cpi_count: u64,
    pub bump: u8,
    pub vault: Pubkey,
}

// ─────────────────────────────────────────────────────────────────────────────
// EVENTS & ERRORS
// ─────────────────────────────────────────────────────────────────────────────
#[event] pub struct HandshakeEvent { pub child_program_id: Pubkey, pub meme_mint: Pubkey, pub deployer: Pubkey }
#[event] pub struct BadgeMinted { pub user: Pubkey, pub badge_mint: Pubkey }
#[event] pub struct GlobalTaxCollected { pub amount_in: u64, pub total_tax: u64, pub user: Pubkey, pub is_buy: bool }
#[event] pub struct RewardsDistributed { pub swapper_sol: u64, pub badge_sol: u64 }
#[event] pub struct AirdropTriggered { pub meme_program_id: Pubkey, pub claimers: u64 }
#[event] pub struct TreasuryWithdrawal { pub amount: u64, pub to: Pubkey }
#[event] pub struct VaultRegistered { pub user: Pubkey, pub vault: Pubkey }

#[error_code]
pub enum SafePumpError {
    #[msg("Already initialized")] AlreadyInitialized,
    #[msg("Not initialized")] NotInitialized,
    #[msg("Invalid supply")] InvalidSupply,
    #[msg("Anti-sniper cooldown")] AntiSniperCooldown,
    #[msg("Math error")] MathError,
    #[msg("Insufficient buy swaps")] InsufficientBuySwaps,
    #[msg("Badge limit reached")] BadgeHolderLimitReached,
    #[msg("Meme coin already registered")] MemeCoinAlreadyRegistered,
    #[msg("Exceeds velocity limit")] ExceedsVelocityLimit,
    #[msg("Invalid BLS signature")] InvalidBlsSignature,
    #[msg("Invalid nonce")] InvalidNonce,
    #[msg("Vault not registered")] VaultNotRegistered,
    #[msg("Distribution period not met")] DistributionPeriodNotMet,
    #[msg("Unauthorized")] Unauthorized,
    #[msg("Sell cooldown not met")] SellCooldownNotMet,
    #[msg("Invalid burn percentage")] InvalidBurnPercentage,
    #[msg("Airdrop already claimed")] AirdropClaimed,
    #[msg("Airdrop limit exceeded")] AirdropLimitExceeded,
    #[msg("Airdrop not triggered")] AirdropNotTriggered,
    #[msg("Mint must end with SPMP")] InvalidMintSuffix,
    #[msg("CPI rate limit")] CpiRateLimit,
}

// ─────────────────────────────────────────────────────────────────────────────
// BLS VERIFICATION
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
        require!(s.ends_with(MEME_MINT_SUFFIX), SafePumpError::InvalidMintSuffix);
    }};
}

// ─────────────────────────────────────────────────────────────────────────────
// PROGRAM
// ─────────────────────────────────────────────────────────────────────────────
#[program]
pub mod safe_pump {
    use super::*;

    pub fn initialize_global(ctx: Context<InitializeGlobal>, treasury_wallet: Pubkey) -> Result<()> {
        let state = &mut ctx.accounts.global_state;
        require!(!state.is_initialized, SafePumpError::AlreadyInitialized);
        state.is_initialized = true;
        state.treasury_wallet = treasury_wallet;
        state.bond_timestamp = Clock::get()?.unix_timestamp;
        state.bump = ctx.bumps.global_state;
        Ok(())
    }

    pub fn handshake(ctx: Context<Handshake>, child_program_id: Pubkey) -> Result<()> {
        require_spmp_suffix!(ctx.accounts.meme_mint);
        let registry = &mut ctx.accounts.registry;
        let entry = (ctx.accounts.meme_mint.key(), child_program_id);
        require!(!registry.entries.contains(&entry), SafePumpError::MemeCoinAlreadyRegistered);
        registry.entries.push(entry);
        emit!(HandshakeEvent { child_program_id, meme_mint: ctx.accounts.meme_mint.key(), deployer: ctx.accounts.deployer.key() });
        Ok(())
    }

    pub fn register_vault(ctx: Context<RegisterVault>) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        vault.bump = ctx.bumps.vault;
        vault.nonce = 0;
        vault.last_signer = Pubkey::default();
        ctx.accounts.user_state.vault = ctx.accounts.vault.key();
        emit!(VaultRegistered { user: ctx.accounts.user.key(), vault: ctx.accounts.vault.key() });
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

        let contract = &ctx.accounts.contract;
        require!(contract.is_initialized, SafePumpError::NotInitialized);

        let clock = Clock::get()?;

        // Anti-sniper on first swap
        if contract.swap_count == 0 {
            require!(clock.unix_timestamp - contract.bond_timestamp >= ANTI_SNIPER_COOLDOWN, SafePumpError::AntiSniperCooldown);
        }

        // User swap cooldown (24h sell cooldown)
        require!(
            clock.unix_timestamp - ctx.accounts.user_state.last_swap_timestamp >= SWAP_COOLDOWN || is_buy,
            SafePumpError::SellCooldownNotMet
        );

        // ZK Vault + BLS verification
        require_keys_eq!(ctx.accounts.vault.key(), ctx.accounts.user_state.vault, SafePumpError::VaultNotRegistered);
        require!(ctx.accounts.vault.nonce == nonce, SafePumpError::InvalidNonce);

        let mut msg = Vec::new();
        msg.extend_from_slice(&amount_in.to_le_bytes());
        msg.push(if is_buy { 1 } else { 0 });
        msg.extend_from_slice(&minimum_amount_out.to_le_bytes());
        msg.extend_from_slice(&nonce.to_le_bytes());
        msg.extend_from_slice(ctx.accounts.user.key().as_ref());
        require!(verify_bls_sig(bls_sig, bls_pk, &msg), SafePumpError::InvalidBlsSignature);

        // === FIRST: Collect global tax via Mothership CPI ===
        let mothership_cpi_accounts = crate::cpi::accounts::GlobalTaxSwap {
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
        };
        let mothership_cpi_ctx = CpiContext::new(ctx.accounts.mothership_program.to_account_info(), mothership_cpi_accounts);
        crate::cpi::global_tax_swap(mothership_cpi_ctx, amount_in, is_buy, minimum_amount_out, bls_sig, bls_pk, nonce)?;

        // === THEN: Perform actual Raydium swap (after tax) ===
        let net_amount = amount_in * (10_000 - GLOBAL_TAX_BPS) / 10_000;

        if is_buy {
            raydium_cp_swap::cpi::swap_base_in(
                CpiContext::new(
                    ctx.accounts.raydium_program.to_account_info(),
                    raydium_cp_swap::cpi::accounts::SwapBaseIn {
                        pool_state: ctx.accounts.pool_state.to_account_info(),
                        user_source_token: ctx.accounts.user_sol.to_account_info(),
                        user_destination_token: ctx.accounts.user_token.to_account_info(),
                        token_0_vault: ctx.accounts.token_vault.to_account_info(),
                        token_1_vault: ctx.accounts.sol_vault.to_account_info(),
                        token_program: ctx.accounts.token_program.to_account_info(),
                        remaining_accounts: vec![],
                    },
                ),
                SwapBaseInput {
                    amount_in: net_amount,
                    minimum_amount_out,
                },
            )?;
        } else {
            raydium_cp_swap::cpi::swap_base_in(
                CpiContext::new(
                    ctx.accounts.raydium_program.to_account_info(),
                    raydium_cp_swap::cpi::accounts::SwapBaseIn {
                        pool_state: ctx.accounts.pool_state.to_account_info(),
                        user_source_token: ctx.accounts.user_token.to_account_info(),
                        user_destination_token: ctx.accounts.user_sol.to_account_info(),
                        token_0_vault: ctx.accounts.token_vault.to_account_info(),
                        token_1_vault: ctx.accounts.sol_vault.to_account_info(),
                        token_program: ctx.accounts.token_program.to_account_info(),
                        remaining_accounts: vec![],
                    },
                ),
                SwapBaseInput {
                    amount_in,
                    minimum_amount_out,
                },
            )?;
        }

        // Update user state
        ctx.accounts.user_state.last_swap_timestamp = clock.unix_timestamp;
        ctx.accounts.vault.nonce = nonce.checked_add(1).ok_or(SafePumpError::MathError)?;

        Ok(())
    }

    pub fn global_tax_swap(
        ctx: Context<GlobalTaxSwap>,
        amount_in: u64,
        is_buy: bool,
        min_out: u64,
        bls_sig: [u8; 96],
        bls_pk: [u8; 48],
        nonce: u64,
    ) -> Result<()> {
        let state = &mut ctx.accounts.global_state;
        require!(state.is_initialized, SafePumpError::NotInitialized);

        let clock = Clock::get()?;
        if state.swap_count == 0 {
            require!(clock.unix_timestamp - state.bond_timestamp >= ANTI_SNIPER_COOLDOWN, SafePumpError::AntiSniperCooldown);
        }

        require_keys_eq!(ctx.accounts.vault.key(), ctx.accounts.expected_vault.key(), SafePumpError::VaultNotRegistered);
        require!(ctx.accounts.vault.nonce == nonce, SafePumpError::InvalidNonce);

        let mut msg = Vec::new();
        msg.extend_from_slice(&amount_in.to_le_bytes());
        msg.push(if is_buy { 1 } else { 0 });
        msg.extend_from_slice(&min_out.to_le_bytes());
        msg.extend_from_slice(&nonce.to_le_bytes());
        msg.extend_from_slice(ctx.accounts.user.key().as_ref());
        require!(verify_bls_sig(bls_sig, bls_pk, &msg), SafePumpError::InvalidBlsSignature);

        let velocity = &mut ctx.accounts.velocity;
        if velocity.block_slot != clock.slot {
            velocity.block_slot = clock.slot;
            velocity.total_bought_sol = 0;
        }
        if is_buy {
            let tier_idx = FIB_MCAP_THRESHOLDS_LAMPORTS.iter().position(|&t| state.total_swapped >= t).unwrap_or(7);
            let limit = FIB_VELOCITY_SOL[tier_idx] * LAMPORTS_PER_SOL;
            velocity.total_bought_sol = velocity.total_bought_sol.checked_add(amount_in).ok_or(SafePumpError::MathError)?;
            require!(velocity.total_bought_sol <= limit, SafePumpError::ExceedsVelocityLimit);
        }

        let total_tax = amount_in * GLOBAL_TAX_BPS / 10_000;
        let lp_tax = total_tax * GLOBAL_LP_TAX_BPS / GLOBAL_TAX_BPS;
        let swapper_tax = total_tax * SWAPPER_REWARD_TAX_BPS / GLOBAL_TAX_BPS;
        let badge_tax = total_tax * BADGE_REWARD_TAX_BPS / GLOBAL_TAX_BPS;
        let treasury_tax = total_tax * TREASURY_TAX_BPS / GLOBAL_TAX_BPS;

        if lp_tax > 0 { token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), Transfer { from: ctx.accounts.user_sol.to_account_info(), to: ctx.accounts.lp_vault.to_account_info(), authority: ctx.accounts.user.to_account_info() }), lp_tax)?; }
        if treasury_tax > 0 { token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), Transfer { from: ctx.accounts.user_sol.to_account_info(), to: ctx.accounts.treasury_vault.to_account_info(), authority: ctx.accounts.user.to_account_info() }), treasury_tax)?; }

        let rewards = &mut ctx.accounts.rewards;
        if rewards.swap_count < MAX_BADGE_HOLDERS as u64 {
            rewards.swapper_rewards[rewards.swap_count as usize] = (ctx.accounts.user.key(), swapper_tax);
            rewards.swap_count += 1;
        }
        rewards.badge_rewards = rewards.badge_rewards.checked_add(badge_tax).ok_or(SafePumpError::MathError)?;

        if is_buy {
            let holders = &mut ctx.accounts.badge_holders;
            let user_key = ctx.accounts.user.key();
            if let Some(pos) = holders.buy_swap_count.iter().position(|(k, _)| *k == user_key) {
                holders.buy_swap_count[pos].1 += 1;
            } else if holders.holder_count < MAX_BADGE_HOLDERS as u64 {
                holders.buy_swap_count[holders.holder_count as usize] = (user_key, 1);
                holders.holder_count += 1;
            }
        }

        state.swap_count += 1;
        state.total_swapped = state.total_swapped.checked_add(amount_in).ok_or(SafePumpError::MathError)?;
        ctx.accounts.vault.nonce = nonce.checked_add(1).ok_or(SafePumpError::MathError)?;

        emit!(GlobalTaxCollected { amount_in, total_tax, user: ctx.accounts.user.key(), is_buy });
        Ok(())
    }



    pub fn distribute_rewards(ctx: Context<DistributeRewards>) -> Result<()> {
        let rewards = &mut ctx.accounts.rewards;
        let clock = Clock::get()?;
        require!(clock.unix_timestamp - rewards.last_distribution_timestamp >= REWARD_DISTRIBUTION_PERIOD, SafePumpError::DistributionPeriodNotMet);

        let mut swapper_sol = 0u64;
        for (user, amt) in rewards.swapper_rewards.iter_mut() {
            if *amt > 0 {
                invoke(&solana_program::system_instruction::transfer(&ctx.accounts.treasury_vault.key(), user, *amt), &[ctx.accounts.treasury_vault.to_account_info(), ctx.accounts.system_program.to_account_info()])?;
                swapper_sol += *amt;
                *amt = 0;
            }
        }

        let badge_sol = if rewards.badge_rewards > 0 && ctx.accounts.badge_holders.holder_count > 0 {
            let per = rewards.badge_rewards / ctx.accounts.badge_holders.holder_count;
            for holder in ctx.accounts.badge_holders.holders.iter().take(ctx.accounts.badge_holders.holder_count as usize) {
                if *holder != Pubkey::default() {
                    invoke(&solana_program::system_instruction::transfer(&ctx.accounts.treasury_vault.key(), holder, per), &[ctx.accounts.treasury_vault.to_account_info(), ctx.accounts.system_program.to_account_info()])?;
                }
            }
            per * ctx.accounts.badge_holders.holder_count
        } else { 0 };

        rewards.swap_count = 0;
        rewards.badge_rewards = 0;
        rewards.last_distribution_timestamp = clock.unix_timestamp;
        emit!(RewardsDistributed { swapper_sol, badge_sol });
        Ok(())
    }

    pub fn mint_badge(ctx: Context<MintBadge>) -> Result<()> {
        let holders = &ctx.accounts.badge_holders;
        let user_key = ctx.accounts.user.key();
        let count = holders.buy_swap_count.iter().find(|(k, _)| *k == user_key).map(|(_, c)| *c).unwrap_or(0);
        require!(count >= BUY_SWAPS_FOR_BADGE, SafePumpError::InsufficientBuySwaps);
        require!(holders.holder_count < MAX_BADGE_HOLDERS as u64, SafePumpError::BadgeHolderLimitReached);

        let (badge_mint, bump) = Pubkey::find_program_address(&[b"badge", ctx.accounts.mint.key().as_ref(), user_key.as_ref()], &crate::id());
        token::mint_to(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo { mint: ctx.accounts.badge_mint.to_account_info(), to: ctx.accounts.user_badge_ata.to_account_info(), authority: ctx.accounts.contract.to_account_info() },
            &[&[b"badge", ctx.accounts.mint.key().as_ref(), user_key.as_ref(), &[bump]]]
        ), 1)?;
        emit!(BadgeMinted { user: user_key, badge_mint });
        Ok(())
    }

    pub fn airdrop_claim(ctx: Context<AirdropClaim>) -> Result<()> {
        let registry = &mut ctx.accounts.airdrop_registry;
        let user_key = ctx.accounts.user.key();
        require!(!registry.claimers.contains(&user_key), SafePumpError::AirdropClaimed);
        require!(registry.claimers.len() < MAX_AIRDROP_CLAIMERS, SafePumpError::AirdropLimitExceeded);
        registry.claimers.push(user_key);
        ctx.accounts.claim_record.claimed = true;
        Ok(())
    }

    pub fn trigger_airdrop(ctx: Context<TriggerAirdrop>, _meme_program_id: Pubkey) -> Result<()> {
        require_spmp_suffix!(ctx.accounts.mint);
        let registry = &ctx.accounts.airdrop_registry;
        require!(registry.claimers.len() >= AIRDROP_TRIGGER_COUNT, SafePumpError::AirdropNotTriggered);

        let total_supply = ctx.accounts.meme_token.total_supply;
        let per_user = total_supply / 100_000;
        let total = registry.claimers.len() as u64 * per_user;
        let seeds = &[b"contract", ctx.accounts.owner.key().as_ref(), &[ctx.accounts.contract.bump]];

        for &claimer in &registry.claimers {
            let ata = associated_token::get_associated_token_address(&claimer, &ctx.accounts.mint.key());
            associated_token::create_associated_token_account(CpiContext::new(ctx.accounts.associated_token_program.to_account_info(), associated_token::Create {
                payer: ctx.accounts.owner.to_account_info(),
                associated_token: ata.into(),
                authority: claimer.into(),
                mint: ctx.accounts.mint.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            }))?;
            token::mint_to(CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo { mint: ctx.accounts.mint.to_account_info(), to: ata.into(), authority: ctx.accounts.contract.to_account_info() },
                &[seeds]
            ), per_user)?;
        }

        ctx.accounts.contract.vault_token_balance = ctx.accounts.contract.vault_token_balance.checked_sub(total).ok_or(SafePumpError::MathError)?;
        emit!(AirdropTriggered { meme_program_id: ctx.accounts.mint.key(), claimers: registry.claimers.len() as u64 });
        Ok(())
    }

    pub fn burn_laser_cpi(ctx: Context<BurnLaserCpi>, percent: u8) -> Result<()> {
        require_spmp_suffix!(ctx.accounts.lp_mint);
        require!(percent <= 50, SafePumpError::InvalidBurnPercentage);
        let burn = ctx.accounts.lp_vault.amount * percent as u64 / 100;
        token::burn(CpiContext::new(ctx.accounts.token_program.to_account_info(), Burn {
            mint: ctx.accounts.lp_mint.to_account_info(),
            from: ctx.accounts.lp_vault.to_account_info(),
            authority: ctx.accounts.contract.to_account_info()
        }), burn)?;
        Ok(())
    }

        pub fn withdraw_treasury(ctx: Context<WithdrawTreasury>, amount: u64) -> Result<()> {
        require!(ctx.accounts.signer.key() == ctx.accounts.global_state.treasury_wallet, SafePumpError::Unauthorized);
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.treasury_vault.to_account_info(),
                    to: ctx.accounts.destination.to_account_info(),
                },
            ),
            amount,
        )?;
        emit!(TreasuryWithdrawal {
            amount,
            to: ctx.accounts.destination.key(),
        });
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // FULL TOKEN LAUNCH + POOL CREATION (child program calls this once)
    // ─────────────────────────────────────────────────────────────────────────
    pub fn initialize_contract(
        ctx: Context<InitializeContract>,
        total_supply: u64,
        treasury_wallet: Pubkey,
        burn_percentage: u8,
        lp_percentage: u8,
        friends_wallets: Vec<Pubkey>,
        friends_amounts: Vec<u64>,
        deployer_amount: u64,
    ) -> Result<()> {
        require_spmp_suffix!(ctx.accounts.mint);
        require!(total_supply <= MAX_SUPPLY, SafePumpError::InvalidSupply);
        require!(burn_percentage <= 50, SafePumpError::InvalidBurnPercentage);
        require!(lp_percentage <= 100, SafePumpError::InvalidLpPercentage);
        require!(friends_wallets.len() <= MAX_FRIENDS_WALLETS, SafePumpError::InvalidFriendsAllocation);
        require!(friends_wallets.len() == friends_amounts.len(), SafePumpError::InvalidFriendsAllocation);

        let total_allocation = deployer_amount + friends_amounts.iter().sum::<u64>();
        let allocation_percent = (total_allocation * 10_000) / total_supply;
        require!(allocation_percent <= MAX_ALLOCATION_PERCENT, SafePumpError::InvalidFriendsAllocation);
        require!(lp_percentage as u64 == 100 - (allocation_percent / 100), SafePumpError::InvalidLpPercentage);

        let contract = &mut ctx.accounts.contract;
        require!(!contract.is_initialized, SafePumpError::AlreadyInitialized);

        contract.is_initialized = true;
        contract.total_supply = total_supply;
        contract.treasury_wallet = treasury_wallet;
        contract.burn_percentage = burn_percentage;
        contract.deployer_amount = deployer_amount;
        contract.bond_timestamp = Clock::get()?.unix_timestamp;
        contract.bump = ctx.bumps.contract;

        for (i, (wallet, amount)) in friends_wallets.iter().zip(friends_amounts.iter()).enumerate() {
            contract.friends_wallets[i] = *wallet;
            contract.friends_amounts[i] = *amount;
        }

        let authority_seeds = &[b"contract", ctx.accounts.deployer.key().as_ref(), &[contract.bump]];

        let mint_to = |to: Pubkey, amount: u64| -> Result<()> {
            let ata = associated_token::get_associated_token_address(&to, &ctx.accounts.mint.key());
            associated_token::create(
                CpiContext::new(
                    ctx.accounts.associated_token_program.to_account_info(),
                    associated_token::Create {
                        payer: ctx.accounts.deployer.to_account_info(),
                        associated_token: ata.into(),
                        authority: to.into(),
                        mint: ctx.accounts.mint.to_account_info(),
                        system_program: ctx.accounts.system_program.to_account_info(),
                        token_program: ctx.accounts.token_program.to_account_info(),
                    },
                ),
            )?;
            token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    MintTo {
                        mint: ctx.accounts.mint.to_account_info(),
                        to: ata.into(),
                        authority: ctx.accounts.contract.to_account_info(),
                    },
                    &[authority_seeds],
                ),
                amount,
            )?;
            Ok(())
        };

        // Deployer + friends allocation (with burn)
        let post_burn = |amt: u64| amt * (100 - burn_percentage as u64) / 100;
        mint_to(ctx.accounts.deployer.key(), post_burn(deployer_amount))?;
        for (wallet, amount) in friends_wallets.iter().zip(friends_amounts.iter()) {
            mint_to(*wallet, post_burn(*amount))?;
        }

        // LP tokens
        let lp_tokens = (total_supply * lp_percentage as u64) / 100;
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.contract.to_account_info(),
                },
                &[authority_seeds],
            ),
            lp_tokens,
        )?;

        // Initial vault reserves
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.badge_vault.to_account_info(),
                    authority: ctx.accounts.contract.to_account_info(),
                },
                &[authority_seeds],
            ),
            INITIAL_VAULT_AMOUNT,
        )?;

        // Transfer SOL for pool
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.deployer_wsol.to_account_info(),
                    to: ctx.accounts.sol_vault.to_account_info(),
                    authority: ctx.accounts.deployer.to_account_info(),
                },
            ),
            POOL_SOL_AMOUNT,
        )?;

        // Create Raydium pool
        create_pool(
            CpiContext::new_with_signer(
                ctx.accounts.raydium_program.to_account_info(),
                CreatePool {
                    pool_state: ctx.accounts.pool_state.to_account_info(),
                    token_0_vault: ctx.accounts.vault.to_account_info(),
                    token_1_vault: ctx.accounts.sol_vault.to_account_info(),
                    lp_mint: ctx.accounts.lp_mint.to_account_info(),
                    amm_config: ctx.accounts.amm_config.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                    observation_state: ctx.accounts.observation_state.to_account_info(),
                    create_pool_fee: ctx.accounts.create_pool_fee.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
                &[authority_seeds],
            ),
        )?;

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ALL CONTEXTS — FULLY MERGED & FINAL
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct InitializeGlobal<'info> {
    #[account(init, payer = authority, space = 8 + 1 + 32 + 8 + 8 + 8 + 1, seeds = [b"global"], bump)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut)] pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Handshake<'info> {
    #[account(mut)] pub deployer: Signer<'info>,
    #[account(mut)] pub meme_mint: Account<'info, Mint>,
    #[account(init_if_needed, payer = deployer, space = 8 + 4 + 2000 * 64 + 1, seeds = [MEME_REGISTRY_SEED], bump)]
    pub registry: Account<'info, MemeCoinRegistry>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct RegisterVault<'info> {
    #[account(init, payer = user, space = 8 + 1 + 8 + 32, seeds = [VAULT_SEED, user.key().as_ref()], bump)]
    pub vault: Account<'info, Vault>,
    #[account(init_if_needed, payer = user, space = 8 + 8 + 8 + 8 + 1 + 32, seeds = [b"user-swap-data", user.key().as_ref()], bump)]
    pub user_state: Account<'info, UserSwapData>,
    #[account(mut)] pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct GlobalTaxSwap<'info> {
    #[account(mut)] pub global_state: Account<'info, GlobalState>,
    #[account(mut)] pub user: Signer<'info>,
    #[account(mut)] pub user_sol: Account<'info, TokenAccount>,
    #[account(mut)] pub vault: Account<'info, Vault>,
    #[account(seeds = [VAULT_SEED, user.key().as_ref()], bump)] pub expected_vault: AccountInfo<'info>,
    #[account(mut)] pub lp_vault: Account<'info, TokenAccount>,
    #[account(mut)] pub treasury_vault: Account<'info, TokenAccount>,
    #[account(mut)] pub rewards: Account<'info, RewardDistribution>,
    #[account(mut)] pub badge_holders: Account<'info, BadgeHolders>,
    #[account(init_if_needed, payer = user, space = 8 + 8 + 8 + 1, seeds = [b"velocity", &Clock::get()?.slot.to_le_bytes()], bump)]
    pub velocity: Account<'info, BlockSwapState>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DistributeRewards<'info> {
    #[account(mut)] pub rewards: Account<'info, RewardDistribution>,
    #[account(mut)] pub badge_holders: Account<'info, BadgeHolders>,
    #[account(mut)] pub treasury_vault: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MintBadge<'info> {
    #[account(mut)] pub user: Signer<'info>,
    #[account(init, payer = user, mint::decimals = 0, mint::authority = contract, seeds = [b"badge", mint.key().as_ref(), user.key().as_ref()], bump)]
    pub badge_mint: Account<'info, Mint>,
    #[account(init_if_needed, payer = user, associated_token::mint = badge_mint, associated_token::authority = user)]
    pub user_badge_ata: Account<'info, TokenAccount>,
    #[account(mut)] pub badge_holders: Account<'info, BadgeHolders>,
    #[account(seeds = [b"contract", deployer.key().as_ref()], bump)]
    pub contract: Account<'info, TokenContract>,
    #[account(mut)] pub mint: Account<'info, Mint>,
    #[account(mut)] pub deployer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawTreasury<'info> {
    #[account(mut)] pub global_state: Account<'info, GlobalState>,
    #[account(mut)] pub treasury_vault: Account<'info, TokenAccount>,
    #[account(mut)] pub destination: AccountInfo<'info>,
    #[account(mut)] pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeContract<'info> {
    #[account(init, payer = deployer, space = 8 + 1 + 8 + 32 + 8 + 8 + 8 + 8 + 1 + 8 + 1 + 1 + 1 + (4 + 32*4) + (4 + 8*4) + 8 + 33 + 33, seeds = [b"contract", deployer.key().as_ref()], bump)]
    pub contract: Account<'info, TokenContract>,
    #[account(mut)] pub deployer: Signer<'info>,
    #[account(mut)] pub mint: Account<'info, Mint>,
    #[account(mut)] pub vault: Account<'info, TokenAccount>,
    #[account(mut)] pub badge_vault: Account<'info, TokenAccount>,
    #[account(mut)] pub sol_vault: Account<'info, TokenAccount>,
    #[account(mut)] pub lp_mint: Account<'info, Mint>,
    #[account(mut)] pub deployer_wsol: Account<'info, TokenAccount>,
    #[account(mut)] pub pool_state: AccountInfo<'info>,
    #[account(mut)] pub observation_state: AccountInfo<'info>,
    #[account(mut)] pub amm_config: AccountInfo<'info>,
    #[account(mut)] pub authority: AccountInfo<'info>,
    #[account(mut)] pub create_pool_fee: AccountInfo<'info>,
    #[account(address = raydium_cp_swap::id())]
    pub raydium_program: Program<'info, raydium_cp_swap::program::RaydiumCpSwap>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct AirdropClaim<'info> {
    #[account(init_if_needed, payer = user, space = 8 + 1 + 1, seeds = [b"airdrop_claim", user.key().as_ref()], bump)]
    pub claim_record: Account<'info, AirdropClaimRecord>,
    #[account(init_if_needed, payer = user, space = 8 + 4 + (32 * MAX_AIRDROP_CLAIMERS) + 1, seeds = [b"airdrop_registry", mint.key().as_ref()], bump)]
    pub airdrop_registry: Account<'info, AirdropRegistry>,
    #[account(mut)] pub user: Signer<'info>,
    #[account(mut)] pub mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TriggerAirdrop<'info> {
    #[account(mut, seeds = [b"contract", owner.key().as_ref()], bump)]
    pub contract: Account<'info, TokenContract>,
    #[account(mut)] pub owner: Signer<'info>,
    #[account(mut, seeds = [b"airdrop_registry", mint.key().as_ref()], bump)]
    pub airdrop_registry: Account<'info, AirdropRegistry>,
    #[account(mut)] pub mint: Account<'info, Mint>,
    #[account(mut)] pub meme_token: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BurnLaserCpi<'info> {
    #[account(mut)] pub lp_vault: Account<'info, TokenAccount>,
    #[account(mut)] pub lp_mint: Account<'info, Mint>,
    #[account(seeds = [b"contract", owner.key().as_ref()], bump)]
    pub contract: Account<'info, TokenContract>,
    #[account(mut)] pub owner: Signer<'info>,
    pub token_program: Program<'info, Token>,
}
#[derive(Accounts)]
pub struct ChildSwap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub clock: Sysvar<'info, Clock>,
    pub user_sol: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token: Account<'info, TokenAccount>,
    #[account(mut)]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub contract: Account<'info, TokenContract>,

    // Raydium
    #[account(mut)]
    pub pool_state: AccountInfo<'info>,
    #[account(mut)]
    pub token_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub sol_vault: Account<'info, TokenAccount>,
    #[account(address = raydium_cp_swap::id())]
    pub raydium_program: Program<'info, raydium_cp_swap::program::RaydiumCpSwap>,

    // Mothership global state
    #[account(mut, address = MOTHERSHIP_PROGRAM_ID_PUBKEY)]
    pub mothership_program: Program<'info, crate::program::SafePump>,
    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut)]
    pub lp_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub treasury_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub rewards: Account<'info, RewardDistribution>,
    #[account(mut)]
    pub badge_holders: Account<'info, BadgeHolders>,
    #[account(mut)]
    pub vault: Account<'info, Vault>,
    #[account(seeds = [VAULT_SEED, user.key().as_ref()], bump)]
    pub expected_vault: AccountInfo<'info>,
    #[account(init_if_needed, payer = user, space = 8 + 8 + 8 + 1, seeds = [b"velocity", &clock.slot.to_le_bytes()], bump)]
    pub velocity: Account<'info, BlockSwapState>,

    #[account(mut)]
    pub user_state: Account<'info, UserSwapData>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
}

pub mod stealth_airdrop_vault;
use stealth_airdrop_vault::*;
