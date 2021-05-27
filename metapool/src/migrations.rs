//-----------------------------
//-----------------------------
//contract main state migration
//-----------------------------

use near_sdk::{near_bindgen};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{UnorderedMap,LookupMap};


//------------------------------------------------
//  OLD Main Contract State for state migrations
//------------------------------------------------
// Note: Because this contract holds a large liquidity-pool, there are no `min_account_balance` required for accounts.None
// accounts are automatically removed (converted to default) where available & staked & shares & meta = 0. see: internal_update_account
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct MetaPoolPrevStateStruct {
    /// Owner's account ID (it will be a DAO on phase II)
    pub owner_account_id: String,

    /// What should be the contract_account_balance according to our internal accounting (if there's extra, it is 30% tx-fees)
    /// This amount increments with attachedNEAR calls (inflow) and decrements with deposit_and_stake calls (outflow)
    /// increments with retrieve_from_staking_pool (inflow) and decrements with user withdrawals from the contract (outflow)
    /// It should match env::balance()
    pub contract_account_balance: u128,

    // Configurable info for [NEP-129](https://github.com/nearprotocol/NEPs/pull/129)
    pub web_app_url: Option<String>, 
    pub auditor_account_id: Option<String>,

    /// This value is equivalent to sum(accounts.available)
    /// This amount increments with user's deposits_into_available and decrements when users stake_from_available
    /// increments with finish_unstake and decrements with withdraw_from_available
    pub total_available: u128,

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
    // when someone "unstakes" she "burns" X shares at current price to recoup Y near
    pub total_stake_shares: u128,

    /// META is the governance token. Total meta minted
    pub total_meta: u128,

    /// The total amount of tokens actually unstaked and in the waiting-delay (the tokens are in the staking pools)
    pub total_unstaked_and_waiting: u128,

    /// The total amount of tokens actually unstaked AND retrieved from the pools (the tokens are here)
    /// It represents funds retrieved from the pools, but waiting for the users to execute withdraw_unstaked()
    /// During distribute_unstake(), If sp.unstaked>0 && sp.epoch_for_withdraw == env::epoch_height then all unstaked funds are retrieved from the sp
    /// When the funds are actually requested by the users, total_actually_unstaked_and_retrieved is decremented
    /// and then total_available and acc.available are incremented
    /// total_available + total_actually_unstaked_and_retrieved must be == to `near state meta.pool.near` + 30% burnt fee
    pub total_actually_unstaked_and_retrieved: u128,

    /// the staking pools will add rewards to the staked amount on each epoch
    /// here we store the accumulated amount only for stats purposes. This amount can only grow
    pub accumulated_staked_rewards: u128,

    /// no auto-staking. true while changing staking pools
    pub staking_paused: bool,

    //user's accounts
    pub accounts: UnorderedMap<String, crate::Account>,

    //list of pools to diversify in
    pub staking_pools: Vec<crate::StakingPoolInfo>,

    //validator loan request
    pub loan_requests: LookupMap<String, crate::VLoanRequest>,

    //The next 3 values define the Liq.Provider fee curve
    // NEAR/stNEAR Liquidity pool fee curve params
    // We assume this pool is always UNBALANCED, there should be more NEAR than stNEAR 99% of the time
    ///NEAR/stNEAR Liquidity target. If the Liquidity reach this amount, the fee reaches nslp_min_discount_basis_points
    pub nslp_liquidity_target: u128, // 150_000*NEAR initially
    ///NEAR/stNEAR Liquidity pool max fee
    pub nslp_max_discount_basis_points: u16, //5% initially
    ///NEAR/stNEAR Liquidity pool min fee
    pub nslp_min_discount_basis_points: u16, //0.5% initially

    //The next 3 values define meta rewards multipliers %. (100 => 1x, 200 => 2x, ...)
    ///for each stNEAR paid staking reward, reward stNEAR holders with g-stNEAR. default:5x. reward META = rewards * mult_pct / 100
    pub staker_meta_mult_pct: u16,
    ///for each stNEAR paid as discount, reward stNEAR sellers with g-stNEAR. default:1x. reward META = discounted * mult_pct / 100
    pub stnear_sell_meta_mult_pct: u16,
    ///for each stNEAR paid as discount, reward LP providers  with g-stNEAR. default:20x. reward META = fee * mult_pct / 100
    pub lp_provider_meta_mult_pct: u16,

    /// Operator account ID (who's in charge to call distribute_xx() on a periodic basis)
    pub operator_account_id: String,
    /// operator_rewards_fee_basis_points. (0.2% default) 100 basis point => 1%. E.g.: owner_fee_basis_points=30 => 0.3% owner's fee
    pub operator_rewards_fee_basis_points: u16,
    /// owner's cut on Liquid Unstake fee (3% default)
    pub operator_swap_cut_basis_points: u16,
    /// Treasury account ID (it will be controlled by a DAO on phase II)
    pub treasury_account_id: String,
    /// treasury cut on Liquid Unstake (25% from the fees by default)
    pub treasury_swap_cut_basis_points: u16,
}


#[near_bindgen]
impl MetaPool {

    //-----------------
    //-- migration called after code upgrade
    ///  For next version upgrades, change this function.
    //-- executed after upgrade to NEW CODE
    //-----------------
    /// This fn WILL be called by this contract from `pub fn upgrade` (started from DAO)
    /// Originally a NOOP implementation. KEEP IT if you haven't changed contract state.
    /// If you have changed state, you need to implement migration from old state (keep the old struct with different name to deserialize it first).
    /// 
    #[init(ignore_state)] //do not auto-load state before this function
    pub fn migrate() -> Self {
        //
        // read state with old struct
        let old: migrations::MetaPoolPrevStateStruct = env::state_read().expect("Old state doesn't exist");
        
        // the migration can only be done by the owner.
        assert_eq!(
            &env::predecessor_account_id(),
            &old.owner_account_id,
            "Can only be called by the owner"
        );

        // Create the new contract state using the data from the old contract state.
        // returns this struct that gets stored as contract state
        return Self { 
            owner_account_id: old.owner_account_id,
            contract_busy:false ,
            staking_paused: old.staking_paused,
            contract_account_balance: old.contract_account_balance,
            reserve_for_unstake_claims: 0,
            total_available: old.total_available,

            //-- ORDERS
            epoch_stake_orders: 0,
            epoch_unstake_orders: 0,
            epoch_last_clearing:0,

            total_for_staking: old.total_for_staking,
            total_actually_staked: old.total_actually_staked,
            total_stake_shares: old.total_stake_shares,
            total_meta: old.total_meta,
            total_unstaked_and_waiting: old.total_unstaked_and_waiting,

            total_unstake_claims: 0,

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

            meta_token_account_id: format!("token.{}", env::current_account_id())
        }
    }
}
