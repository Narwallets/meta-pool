//-----------------------------
//-----------------------------
//contract main state migration
//-----------------------------

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap};
use near_sdk::{env, near_bindgen, AccountId, EpochHeight};

use crate::*;

//---------------------------------------------------
//  PREVIOUS Main Contract State for state migrations
//---------------------------------------------------
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct OldMetaPool {
    /// Owner's account ID (it will be a DAO on phase II)
    pub owner_account_id: AccountId,

    /// Avoid re-entry when async-calls are in-flight
    pub contract_busy: bool,

    /// no auto-staking. true while changing staking pools
    pub staking_paused: bool,

    /// What should be the contract_account_balance according to our internal accounting (if there's extra, it is 30% tx-fees)
    /// This amount increments with attachedNEAR calls (inflow) and decrements with deposit_and_stake calls (outflow)
    /// increments with retrieve_from_staking_pool (inflow) and decrements with user withdrawals from the contract (outflow)
    /// It should match env::balance()
    pub contract_account_balance: u128,

    /// Every time a user performs a delayed-unstake, stNEAR tokens are burned and the user gets a unstaked_claim that will
    /// be fulfilled 4 epochs from now. If there are someone else staking in the same epoch, both orders (stake & d-unstake) cancel each other
    /// (no need to go to the staking-pools) but the NEAR received for staking must be now reserved for the unstake-withdraw 4 epochs form now.
    /// This amount increments *after* end_of_epoch_clearing, *if* there are staking & unstaking orders that cancel each-other.
    /// This amount also increments at retrieve_from_staking_pool
    /// The funds here are *reserved* fro the unstake-claims and can only be user to fulfill those claims
    /// This amount decrements at unstake-withdraw, sending the NEAR to the user
    /// Note: There's a extra functionality (quick-exit) that can speed-up unstaking claims if there's funds in this amount.
    pub reserve_for_unstake_claims: u128,

    /// This value is equivalent to sum(accounts.available)
    /// This amount increments with user's deposits_into_available and decrements when users stake_from_available
    /// increments with unstake_to_available and decrements with withdraw_from_available
    /// Note: in the current simplified UI user-flow of the meta-pool, only the NSLP & the treasury can have available balance
    /// the rest of the users mov directly between their NEAR native accounts & the contract accounts, only briefly occupying acc.available
    pub total_available: u128,

    //-- ORDERS
    /// The total amount of "stake" orders in the current epoch
    pub epoch_stake_orders: u128,
    /// The total amount of "delayed-unstake" orders in the current epoch
    pub epoch_unstake_orders: u128,
    // this two amounts can cancel each other at end_of_epoch_clearing
    /// The epoch when the last end_of_epoch_clearing was performed. To avoid calling it twice in the same epoch.
    pub epoch_last_clearing: EpochHeight,

    /// The total amount of tokens selected for staking by the users
    /// not necessarily what's actually staked since staking can is done in batches
    /// Share price is computed using this number. share_price = total_for_staking/total_shares
    pub total_for_staking: u128,

    /// The total amount of tokens actually staked (the tokens are in the staking pools)
    // During distribute_staking(), If !staking_paused && total_for_staking<total_actually_staked, then the difference gets staked in the pools
    // During distribute_unstaking(), If total_actually_staked>total_for_staking, then the difference gets unstaked from the pools
    pub total_actually_staked: u128,

    /// how many "shares" were minted. Every time someone "stakes" he "buys pool shares" with the staked amount
    // the buy share price is computed so if she "sells" the shares on that moment she recovers the same near amount
    // staking produces rewards, rewards are added to total_for_staking so share_price will increase with rewards
    // share_price = total_for_staking/total_shares
    // when someone "unstakes" they "burns" X shares at current price to recoup Y near
    pub total_stake_shares: u128, //total stNEAR minted

    /// META is the governance token. Total meta minted
    pub total_meta: u128,

    /// The total amount of tokens actually unstaked and in the waiting-delay (the tokens are in the staking pools)
    pub total_unstaked_and_waiting: u128,

    /// sum(accounts.unstake). Every time a user delayed-unstakes, this amount is incremented
    /// when the funds are withdrawn the amount is decremented.
    /// Control: total_unstaked_claims == reserve_for_unstaked_claims + total_unstaked_and_waiting
    pub total_unstake_claims: u128,

    /// the staking pools will add rewards to the staked amount on each epoch
    /// here we store the accumulated amount only for stats purposes. This amount can only grow
    pub accumulated_staked_rewards: u128,

    //user's accounts
    pub accounts: UnorderedMap<AccountId, Account>,

    //list of pools to diversify in
    pub staking_pools: Vec<StakingPoolInfo>,

    // validator loan request
    // action on audit suggestions, this field is not used. No need for this to be on the main contract
    pub loan_requests: LookupMap<AccountId, VLoanRequest>,

    //The next 3 values define the Liq.Provider fee curve
    // NEAR/stNEAR Liquidity pool fee curve params
    // We assume this pool is always UNBALANCED, there should be more NEAR than stNEAR 99% of the time
    ///NEAR/stNEAR Liquidity target. If the Liquidity reach this amount, the fee reaches nslp_min_discount_basis_points
    pub nslp_liquidity_target: u128, // 150_000*NEAR initially
    ///NEAR/stNEAR Liquidity pool max fee
    pub nslp_max_discount_basis_points: u16, //5% initially
    ///NEAR/stNEAR Liquidity pool min fee
    pub nslp_min_discount_basis_points: u16, //0.5% initially

    //The next 3 values define meta rewards multipliers. (10 => 1x, 20 => 2x, ...)
    ///for each stNEAR paid staking reward, reward stNEAR holders with META. default:5x. reward META = rewards * (mult_pct*10) / 100
    pub staker_meta_mult_pct: u16,
    ///for each stNEAR paid as discount, reward stNEAR sellers with META. default:1x. reward META = discounted * (mult_pct*10) / 100
    pub stnear_sell_meta_mult_pct: u16,
    ///for each stNEAR paid as discount, reward LP providers  with META. default:20x. reward META = fee * (mult_pct*10) / 100
    pub lp_provider_meta_mult_pct: u16,

    /// min amount accepted as deposit or stake
    pub min_deposit_amount: u128,

    /// Operator account ID (who's in charge to call distribute_xx() on a periodic basis)
    pub operator_account_id: AccountId,
    /// operator_rewards_fee_basis_points. (0.2% default) 100 basis point => 1%. E.g.: owner_fee_basis_points=30 => 0.3% owner's fee
    pub operator_rewards_fee_basis_points: u16,
    /// owner's cut on Liquid Unstake fee (3% default)
    pub operator_swap_cut_basis_points: u16,
    /// Treasury account ID (it will be controlled by a DAO on phase II)
    pub treasury_account_id: AccountId,
    /// treasury cut on Liquid Unstake (25% from the fees by default)
    pub treasury_swap_cut_basis_points: u16,

    // Configurable info for [NEP-129](https://github.com/nearprotocol/NEPs/pull/129)
    pub web_app_url: Option<String>,
    pub auditor_account_id: Option<AccountId>,

    /// Where's the NEP-141 $META token contract
    pub meta_token_account_id: AccountId,

    /// estimated & max meta rewards for each category
    pub est_meta_rewards_stakers: u128,
    pub est_meta_rewards_lu: u128, //liquid-unstakers
    pub est_meta_rewards_lp: u128, //liquidity-providers
    // max. when this amount is passed, corresponding multiplier is damped proportionally
    pub max_meta_rewards_stakers: u128,
    pub max_meta_rewards_lu: u128, //liquid-unstakers
    pub max_meta_rewards_lp: u128, //liquidity-providers
}

use crate::MetaPool;
use crate::MetaPoolContract;

#[near_bindgen]
impl MetaPool {
    //-----------------
    //-- migration called after code upgrade
    ///  For next version upgrades, change this function.
    //-- executed after upgrade to NEW CODE
    //-----------------
    /// This fn WILL be called by this contract from `pub fn upgrade` (started from DAO)
    /// Originally a **NOOP implementation. KEEP IT if you haven't changed contract state.**
    /// If you have changed state, you need to implement migration from old state (keep the old struct with different name to deserialize it first).
    ///
    #[init(ignore_state)] //do not auto-load state before this function
    pub fn migrate() -> Self {
        // read state with OLD struct
        // uncomment when state migration is required on upgrade
        let old: OldMetaPool = env::state_read().expect("Old state doesn't exist");

        // can only be called by this same contract (it's called from fn upgrade())
        assert_eq!(
            &env::predecessor_account_id(),
            &env::current_account_id(),
            "Can only be called by this contract"
        );

        // Create the new contract state using the data from the old contract state.
        // returns this struct that gets stored as contract state
        return Self {
            owner_account_id: old.owner_account_id,
            contract_busy: false,
            staking_paused: old.staking_paused,
            contract_account_balance: old.contract_account_balance,
            reserve_for_unstake_claims: old.reserve_for_unstake_claims,
            total_available: old.total_available,

            //-- ORDERS
            epoch_stake_orders: old.epoch_stake_orders,
            epoch_unstake_orders: old.epoch_unstake_orders,
            epoch_last_clearing: old.epoch_last_clearing,

            total_for_staking: old.total_for_staking,
            total_actually_staked: old.total_actually_staked,
            total_stake_shares: old.total_stake_shares,
            total_meta: old.total_meta,
            total_unstaked_and_waiting: old.total_unstaked_and_waiting,

            total_unstake_claims: old.total_unstake_claims,

            accumulated_staked_rewards: old.accumulated_staked_rewards,

            accounts: old.accounts,

            staking_pools: old.staking_pools,

            loan_requests: old.loan_requests,

            nslp_liquidity_target: old.nslp_liquidity_target,
            nslp_max_discount_basis_points: old.nslp_max_discount_basis_points,
            nslp_min_discount_basis_points: old.nslp_min_discount_basis_points,

            staker_meta_mult_pct: old.staker_meta_mult_pct,
            stnear_sell_meta_mult_pct: old.stnear_sell_meta_mult_pct,
            lp_provider_meta_mult_pct: old.lp_provider_meta_mult_pct,

            operator_account_id: old.operator_account_id,
            operator_rewards_fee_basis_points: old.operator_rewards_fee_basis_points,
            operator_swap_cut_basis_points: old.operator_swap_cut_basis_points,

            treasury_account_id: old.treasury_account_id,
            treasury_swap_cut_basis_points: old.treasury_swap_cut_basis_points,

            // Configurable info for [NEP-129](https://github.com/nearprotocol/NEPs/pull/129)
            web_app_url: old.web_app_url,
            auditor_account_id: old.auditor_account_id,

            meta_token_account_id: old.meta_token_account_id,
            min_deposit_amount: old.min_deposit_amount,

            est_meta_rewards_stakers: old.est_meta_rewards_stakers,
            est_meta_rewards_lu: old.est_meta_rewards_lu,
            est_meta_rewards_lp: old.est_meta_rewards_lp,
            max_meta_rewards_stakers: old.max_meta_rewards_stakers,
            max_meta_rewards_lu: old.max_meta_rewards_lu,
            max_meta_rewards_lp: old.max_meta_rewards_lp,
        };
    }
}
