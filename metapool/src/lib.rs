//! A smart contract that allows diversified staking, stNEAR and META farming
//! this contract include parts of core-contracts/lockup-contract & core-contracts/staking-pool

/********************************/
/* CONTRACT Self Identification */
/********************************/
// [NEP-129](https://github.com/nearprotocol/NEPs/pull/129)
// see also pub fn get_contract_info
const CONTRACT_NAME: &str = "Metapool";
const CONTRACT_VERSION: &str = "0.1.6";
const DEFAULT_WEB_APP_URL: &str = "https://metapool.app";
const DEFAULT_AUDITOR_ACCOUNT_ID: &str = "auditors.near";

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap};
use near_sdk::json_types::Base58PublicKey;
use near_sdk::{env, ext_contract, log, near_bindgen, AccountId, PanicOnDefault, Promise};

//-- Sputnik DAO remote upgrade requires BLOCKCHAIN_INTERFACE low-level access
#[cfg(target_arch = "wasm32")]
use near_sdk::env::BLOCKCHAIN_INTERFACE;

pub mod gas;
pub mod types;
pub mod utils;
pub use crate::owner::*;
pub use crate::types::*;
pub use crate::utils::*;

pub mod account;
pub mod internal;
pub mod staking_pools;
pub use crate::account::*;
pub use crate::internal::*;
pub use crate::staking_pools::*;

pub mod distribute;
mod migrations;
pub mod owner;

pub mod reward_meter;
pub use reward_meter::*;

pub mod validator_loans;
pub use validator_loans::*;

pub mod empty_nep_145;
pub mod fungible_token_standard;

//mod migrations;

// setup_alloc adds a #[cfg(target_arch = "wasm32")] to the global allocator, which prevents the allocator
// from being used when the contract's main file is used in simulation testing.
near_sdk::setup_alloc!();

//self-callbacks
#[ext_contract(ext_self_owner)]
pub trait ExtMetaStakingPoolOwnerCallbacks {
    fn on_staking_pool_deposit(&mut self, amount: U128String) -> bool;

    fn on_retrieve_from_staking_pool(&mut self, inx: u16) -> bool;

    fn on_staking_pool_stake_maybe_deposit(
        &mut self,
        sp_inx: usize,
        amount: u128,
        included_deposit: bool,
    ) -> bool;

    fn on_staking_pool_unstake(&mut self, sp_inx: usize, amount: U128String) -> bool;

    fn on_get_result_from_transfer_poll(&mut self, #[callback] poll_result: PollResult) -> bool;

    fn on_get_sp_total_balance(&mut self, sp_inx: usize, #[callback] total_balance: U128String);

    fn on_get_sp_unstaked_balance(
        &mut self,
        sp_inx: usize,
        #[callback] unstaked_balance: U128String,
    );

    fn after_minting_meta(self, account_id: AccountId, to_mint: U128String);
}

#[ext_contract(meta_token_mint)]
pub trait MetaToken {
    fn mint(&mut self, account_id: AccountId, amount: U128String);
}

//------------------------
//  Main Contract State --
//------------------------
// Note: Because this contract holds a large liquidity-pool, there are no `min_account_balance` required for accounts.
// Accounts are automatically removed (converted to default) where available & staked & shares & meta = 0. see: internal_update_account
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct MetaPool {
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
    /// The funds here are *reserved* for the unstake-claims and can only be used to fulfill those claims
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

#[near_bindgen]
impl MetaPool {
    /* NOTE
    This contract implements several traits

    1. core-contracts/staking-pool: this contract must be perceived as a staking-pool for the lockup-contract, wallets, and users.
        This means implementing: ping, deposit, deposit_and_stake, withdraw_all, withdraw, stake_all, stake, unstake_all, unstake
        and view methods: get_account_unstaked_balance, get_account_staked_balance, get_account_total_balance, is_account_unstaked_balance_available,
            get_total_staked_balance, get_owner_id, get_reward_fee_fraction, is_staking_paused, get_staking_key, get_account,
            get_number_of_accounts, get_accounts.

    2. meta-staking: these are the extensions to the standard staking pool (liquid_unstake, trip-meter)

    3. fungible token [NEP-141]: this contract is the NEP-141 contract for the stNEAR token

    4. multi fungible token [NEP-138]: this contract works as a multi-token also to allow access to the $META governance token

    */

    /// Initializes MetaPool contract.
    /// - `owner_account_id` - the account ID of the owner.  Only this account can call owner's methods on this contract.
    #[init]
    pub fn new(
        owner_account_id: AccountId,
        treasury_account_id: AccountId,
        operator_account_id: AccountId,
        meta_token_account_id: AccountId,
    ) -> Self {
        assert!(!env::state_exists(), "The contract is already initialized");

        let result = Self {
            owner_account_id,
            contract_busy: false,
            operator_account_id,
            treasury_account_id,
            contract_account_balance: 0,
            web_app_url: Some(String::from(DEFAULT_WEB_APP_URL)),
            auditor_account_id: Some(String::from(DEFAULT_AUDITOR_ACCOUNT_ID)),
            operator_rewards_fee_basis_points: DEFAULT_OPERATOR_REWARDS_FEE_BASIS_POINTS,
            operator_swap_cut_basis_points: DEFAULT_OPERATOR_SWAP_CUT_BASIS_POINTS,
            treasury_swap_cut_basis_points: DEFAULT_TREASURY_SWAP_CUT_BASIS_POINTS,
            staking_paused: false,
            total_available: 0,
            total_for_staking: 0,
            total_actually_staked: 0,
            total_unstaked_and_waiting: 0,
            reserve_for_unstake_claims: 0,
            total_unstake_claims: 0,
            epoch_stake_orders: 0,
            epoch_unstake_orders: 0,
            epoch_last_clearing: 0,
            accumulated_staked_rewards: 0,
            total_stake_shares: 0,
            total_meta: 0,
            accounts: UnorderedMap::new(b"A".to_vec()),
            loan_requests: LookupMap::new(b"L".to_vec()),
            nslp_liquidity_target: 10_000 * NEAR,
            nslp_max_discount_basis_points: 180, //1.8%
            nslp_min_discount_basis_points: 25,  //0.25%
            min_deposit_amount: 10 * NEAR,
            ///for each stNEAR paid as discount, reward stNEAR sellers with META. initial 5x, default:1x. reward META = discounted * mult_pct / 100
            stnear_sell_meta_mult_pct: 50, //5x
            ///for each stNEAR paid staking reward, reward stNEAR holders with META. initial 10x, default:5x. reward META = rewards * mult_pct / 100
            staker_meta_mult_pct: 5000, //500x
            ///for each stNEAR paid as discount, reward LPs with META. initial 50x, default:20x. reward META = fee * mult_pct / 100
            lp_provider_meta_mult_pct: 200, //20x
            staking_pools: Vec::new(),
            meta_token_account_id,
            est_meta_rewards_stakers: 0,
            est_meta_rewards_lu: 0,
            est_meta_rewards_lp: 0,
            max_meta_rewards_stakers: 1_000_000 * ONE_NEAR,
            max_meta_rewards_lu: 50_000 * ONE_NEAR,
            max_meta_rewards_lp: 100_000 * ONE_NEAR,
        };
        //all key accounts must be different
        result.assert_key_accounts_are_different();
        return result;
    }

    fn assert_key_accounts_are_different(&self) {
        //all accounts must be different
        assert!(self.owner_account_id != self.operator_account_id);
        assert!(self.owner_account_id != DEVELOPERS_ACCOUNT_ID);
        assert!(self.owner_account_id != self.treasury_account_id);
        assert!(self.operator_account_id != DEVELOPERS_ACCOUNT_ID);
        assert!(self.operator_account_id != self.treasury_account_id);
        assert!(self.treasury_account_id != DEVELOPERS_ACCOUNT_ID);
    }

    //------------------------------------
    // core-contracts/staking-pool trait
    //------------------------------------

    /// staking-pool's ping is moot here
    pub fn ping(&mut self) {}

    /// Deposits the attached amount into the inner account of the predecessor.
    #[payable]
    pub fn deposit(&mut self) {
        //block "deposit" only, so all actions are thru the simplified user-flow, using deposit_and_stake
        panic!("please use deposit_and_stake");
        //self.internal_deposit();
    }

    /// Withdraws from "UNSTAKED" balance *TO MIMIC core-contracts/staking-pool* .- core-contracts/staking-pool only has "unstaked" to withdraw from
    pub fn withdraw(&mut self, amount: U128String) -> Promise {
        // NOTE: While ability to withdraw close to all available helps, it prevents lockup contracts from using this in a replacement to a staking pool,
        // because the lockup contracts relies on exact precise amount being withdrawn.
        self.internal_withdraw_use_unstaked(amount.0)
    }
    /// Withdraws ALL from from "UNSTAKED" balance *TO MIMIC core-contracts/staking-pool .- core-contracts/staking-pool only has "unstaked" to withdraw from
    pub fn withdraw_all(&mut self) -> Promise {
        let account = self.internal_get_account(&env::predecessor_account_id());
        self.internal_withdraw_use_unstaked(account.unstaked)
    }

    /// user method - simplified flow
    /// completes delayed-unstake action by transferring from retrieved_from_the_pools to user's NEAR account
    /// equivalent to core-contracts/staking-pool.withdraw_all
    pub fn withdraw_unstaked(&mut self) -> Promise {
        let account = self.internal_get_account(&env::predecessor_account_id());
        self.internal_withdraw_use_unstaked(account.unstaked)
    }

    /// meta-pool extension: Withdraws from "available" balance
    pub fn withdraw_from_available(&mut self, amount: U128String) -> Promise {
        self.internal_withdraw_from_available(amount.into())
    }

    /// Deposits the attached amount into the inner account of the predecessor and stakes it.
    #[payable]
    pub fn deposit_and_stake(&mut self) {
        self.internal_deposit();
        self.internal_stake(env::attached_deposit());
    }

    /// Stakes all "unstaked" balance from the inner account of the predecessor.
    /// here we keep the staking-pool logic because we're implementing the staking-pool trait
    pub fn stake_all(&mut self) {
        let account_id = env::predecessor_account_id();
        let account = self.internal_get_account(&account_id);
        self.internal_stake(account.unstaked);
    }

    /// Stakes the given amount from the inner account of the predecessor.
    /// The inner account should have enough unstaked balance.
    pub fn stake(&mut self, amount: U128String) {
        self.internal_stake(amount.0);
    }

    /// Unstakes all staked balance from the inner account of the predecessor.
    /// The new total unstaked balance will be available for withdrawal in four epochs.
    pub fn unstake_all(&mut self) {
        let account_id = env::predecessor_account_id();
        let account = self.internal_get_account(&account_id);
        let amount = self.amount_from_stake_shares(account.stake_shares);
        self.internal_unstake(amount);
    }

    /// Unstakes the given amount from the inner account of the predecessor.
    /// The inner account should have enough staked balance.
    /// The new total unstaked balance will be available for withdrawal in four epochs.
    pub fn unstake(&mut self, amount: U128String) {
        self.internal_unstake(amount.0);
    }

    /*****************************/
    /* staking-pool View methods */
    /*****************************/

    /// Returns the unstaked balance of the given account.
    pub fn get_account_unstaked_balance(&self, account_id: AccountId) -> U128String {
        //warning: self.get_account is public and gets HumanReadableAccount .- do not confuse with self.internal_get_account
        return self.get_account(account_id).unstaked_balance;
    }

    /// Returns the staked balance of the given account.
    /// NOTE: This is computed from the amount of "stake" shares the given account has and the
    /// current amount of total staked balance and total stake shares on the account.
    pub fn get_account_staked_balance(&self, account_id: AccountId) -> U128String {
        //warning: self.get_account is public and gets HumanReadableAccount .- do not confuse with self.internal_get_account
        return self.get_account(account_id).staked_balance;
    }

    /// Returns the total balance of the given account (including staked and unstaked balances).
    pub fn get_account_total_balance(&self, account_id: AccountId) -> U128String {
        let acc = self.internal_get_account(&account_id);
        return (acc.available + self.amount_from_stake_shares(acc.stake_shares) + acc.unstaked)
            .into();
    }

    /// additional to staking-pool to satisfy generic deposit-NEP-standard
    /// returns the amount that can be withdrawn immediately
    pub fn get_account_available_balance(&self, account_id: AccountId) -> U128String {
        let acc = self.internal_get_account(&account_id);
        return acc.available.into();
    }

    /// Returns `true` if the given account can withdraw tokens in the current epoch.
    pub fn is_account_unstaked_balance_available(&self, account_id: AccountId) -> bool {
        //warning: self.get_account is public and gets HumanReadableAccount .- do not confuse with self.internal_get_account
        return self.get_account(account_id).can_withdraw;
    }

    /// Returns account ID of the staking pool owner.
    pub fn get_owner_id(&self) -> AccountId {
        return self.owner_account_id.clone();
    }

    /// Returns the current reward fee as a fraction.
    pub fn get_reward_fee_fraction(&self) -> RewardFeeFraction {
        return RewardFeeFraction {
            numerator: (self.operator_rewards_fee_basis_points
                + DEVELOPERS_REWARDS_FEE_BASIS_POINTS)
                .into(),
            denominator: 10_000,
        };
    }

    // idem previous function but in basis_points
    #[payable]
    pub fn set_reward_fee(&mut self, basis_points: u16) {
        self.assert_owner_calling();
        assert!(basis_points < 1000); // less than 10%
                                      // DEVELOPERS_REWARDS_FEE_BASIS_POINTS is included
        self.operator_rewards_fee_basis_points =
            basis_points.saturating_sub(DEVELOPERS_REWARDS_FEE_BASIS_POINTS);
    }

    /// Returns the staking public key
    pub fn get_staking_key(&self) -> Base58PublicKey {
        panic!("no specific staking key for the div-pool");
    }

    /// Returns true if the staking is paused
    pub fn is_staking_paused(&self) -> bool {
        return self.staking_paused;
    }

    /// to implement the Staking-pool interface, get_account returns the same as the staking-pool returns
    /// full account info can be obtained by calling: pub fn get_account_info(&self, account_id: AccountId) -> GetAccountInfoResult
    /// Returns human readable representation of the account for the given account ID.
    //warning: self.get_account is public and gets HumanReadableAccount .- do not confuse with self.internal_get_account
    pub fn get_account(&self, account_id: AccountId) -> HumanReadableAccount {
        let account = self.internal_get_account(&account_id);
        return HumanReadableAccount {
            account_id,
            unstaked_balance: account.unstaked.into(),
            staked_balance: self.amount_from_stake_shares(account.stake_shares).into(),
            can_withdraw: env::epoch_height() >= account.unstaked_requested_unlock_epoch,
        };
    }

    /// Returns the number of accounts that have positive balance on this staking pool.
    pub fn get_number_of_accounts(&self) -> u64 {
        return self.accounts.len();
    }

    /// Returns the list of accounts (staking-pool trait)
    //warning: self.get_accounts is public and gets HumanReadableAccount .- do not confuse with self.internal_get_account
    pub fn get_accounts(&self, from_index: u64, limit: u64) -> Vec<HumanReadableAccount> {
        let keys = self.accounts.keys_as_vector();
        return (from_index..std::cmp::min(from_index + limit, keys.len()))
            .map(|index| self.get_account(keys.get(index).unwrap()))
            .collect();
    }

    //----------------------------------
    //----------------------------------
    // META-STAKING-POOL trait
    //----------------------------------
    //----------------------------------

    /// Returns the list of accounts with full data (div-pool trait)
    pub fn get_accounts_info(&self, from_index: u64, limit: u64) -> Vec<GetAccountInfoResult> {
        let keys = self.accounts.keys_as_vector();
        return (from_index..std::cmp::min(from_index + limit, keys.len()))
            .map(|index| self.get_account_info(keys.get(index).unwrap()))
            .collect();
    }

    /* DEPRECATED in favor of the simplified flow
    /// user method - COMPLEX FLOW
    /// completes delayed-unstake action by moving from retrieved_from_the_pools to *available*
    /// all within the contract
    pub fn finish_unstaking(&mut self) {

        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);

        account.in_memory_try_finish_unstaking(self);

        self.internal_update_account(&account_id, &account);

        log!("@{} finishing unstaking. New available balance is {}",
            account_id, account.available);
    }
    */

    //---------------------------
    // NSLP Methods
    //---------------------------

    /// user method - NEAR/stNEAR SWAP functions
    /// return how much NEAR you can get by selling x stNEAR
    pub fn get_near_amount_sell_stnear(&self, stnear_to_sell: U128String) -> U128String {
        let lp_account = self.internal_get_nslp_account();
        return self
            .internal_get_near_amount_sell_stnear(lp_account.available, stnear_to_sell.0)
            .into();
    }

    /// NEAR/stNEAR Liquidity Pool
    /// computes the discount_basis_points for NEAR/stNEAR Swap based on NSLP Balance
    /// If you want to sell x stNEAR
    pub fn nslp_get_discount_basis_points(&self, stnear_to_sell: U128String) -> u16 {
        let lp_account = self.internal_get_nslp_account();
        return self.internal_get_discount_basis_points(lp_account.available, stnear_to_sell.0);
    }

    /// user method
    /// swaps stNEAR->NEAR in the Liquidity Pool
    /// returns nears transferred
    //#[payable]
    pub fn liquid_unstake(
        &mut self,
        st_near_to_burn: U128String,
        min_expected_near: U128String,
    ) -> LiquidUnstakeResult {
        self.assert_not_busy();
        // Q: Why not? - R: liquid_unstake It's not as problematic as transfer, because it moves tokens between accounts of the same user
        // so let's remove the one_yocto_requirement, waiting for a better solution for the function-call keys NEP-141 problem
        //assert_one_yocto();

        let account_id = env::predecessor_account_id();
        let mut user_account = self.internal_get_account(&account_id);

        let stnear_owned = user_account.stake_shares;

        let st_near_to_sell:u128 =
        // if the amount is close to user's total, remove user's total
        // to: a) do not leave less than ONE_MILLI_NEAR in the account, b) Allow 10 yoctos of rounding, e.g. remove(100) removes 99.999993 without panicking
        if is_close(st_near_to_burn.0, stnear_owned) { // allow for rounding simplification
            stnear_owned
        }
        else  {
            st_near_to_burn.0
        };

        debug!(
            "st_near owned:{}, to_sell:{}",
            user_account.stake_shares, st_near_to_sell
        );

        assert!(
            stnear_owned >= st_near_to_sell,
            "Not enough stNEAR. You own {}",
            stnear_owned
        );

        let mut nslp_account = self.internal_get_nslp_account();

        //compute how many nears are the st_near valued at
        let nears_out = self.amount_from_stake_shares(st_near_to_sell);
        let swap_fee_basis_points =
            self.internal_get_discount_basis_points(nslp_account.available, nears_out);
        assert!(swap_fee_basis_points < 10000, "inconsistency d>1");
        let fee = apply_pct(swap_fee_basis_points, nears_out);

        let near_to_receive = nears_out - fee;
        assert!(
            near_to_receive >= min_expected_near.0,
            "Price changed, your min amount {} is not satisfied {}. Try again",
            min_expected_near.0,
            near_to_receive
        );
        assert!(
            nslp_account.available >= near_to_receive,
            "Not enough liquidity in the liquidity pool"
        );

        //the NEAR for the user comes from the LP
        nslp_account.available -= near_to_receive;
        user_account.available += near_to_receive;

        // keep track of meta rewards for LPs
        self.est_meta_rewards_lp += damp_multiplier(
            fee,
            self.lp_provider_meta_mult_pct,
            self.est_meta_rewards_lp,
            self.max_meta_rewards_lp,
        );

        // compute how many shares the swap fee represent
        let fee_in_st_near = self.stake_shares_from_amount(fee);

        // involved accounts
        assert!(
            &account_id != &self.treasury_account_id,
            "can't use treasury account"
        );
        let mut treasury_account = self.internal_get_account(&self.treasury_account_id);
        assert!(
            &account_id != &self.operator_account_id,
            "can't use operator account"
        );
        let mut operator_account = self.internal_get_account(&self.operator_account_id);
        assert!(
            &account_id != &DEVELOPERS_ACCOUNT_ID,
            "can't use developers account"
        );
        let mut developers_account = self.internal_get_account(&DEVELOPERS_ACCOUNT_ID.into());

        // The treasury cut in stnear-shares (25% by default)
        let treasury_st_near_cut = apply_pct(self.treasury_swap_cut_basis_points, fee_in_st_near);
        treasury_account.add_st_near(treasury_st_near_cut, &self);

        // The cut that the contract owner (operator) takes. (3% of 1% normally)
        let operator_st_near_cut = apply_pct(self.operator_swap_cut_basis_points, fee_in_st_near);
        operator_account.add_st_near(operator_st_near_cut, &self);

        // The cut that the developers take. (2% of 1% normally)
        let developers_st_near_cut = apply_pct(DEVELOPERS_SWAP_CUT_BASIS_POINTS, fee_in_st_near);
        developers_account.add_st_near(developers_st_near_cut, &self);

        // all the realized meta from non-liq.provider cuts (30%), send to operator & developers
        let st_near_non_lp_cut =
            treasury_st_near_cut + operator_st_near_cut + developers_st_near_cut;
        let meta_from_operation = damp_multiplier(
            st_near_non_lp_cut,
            self.lp_provider_meta_mult_pct,
            self.est_meta_rewards_lp,
            self.max_meta_rewards_lp,
        );
        self.total_meta += meta_from_operation;
        operator_account.realized_meta += meta_from_operation / 2;
        developers_account.realized_meta += meta_from_operation / 2;

        debug!("treasury_st_near_cut:{} operator_st_near_cut:{} developers_st_near_cut:{} fee_in_st_near:{}",
            treasury_st_near_cut,operator_st_near_cut,developers_st_near_cut,fee_in_st_near);

        assert!(
            fee_in_st_near > treasury_st_near_cut + developers_st_near_cut + operator_st_near_cut
        );

        // The rest of the st_near sold goes into the liq-pool. Because it is a larger amount than NEARs removed, it will increase share value for all LP providers.
        // Adding value to the pool via adding more stNEAR value than the NEAR removed, will be counted as rewards for the nslp_meter,
        // so $META for LP providers will be created. $METAs for LP providers are realized during add_liquidity(), remove_liquidity()
        let st_near_to_liq_pool = st_near_to_sell
            - (treasury_st_near_cut + operator_st_near_cut + developers_st_near_cut);
        debug!("nslp_account.add_st_near {}", st_near_to_liq_pool);
        // major part of stNEAR sold goes to the NSLP
        nslp_account.add_st_near(st_near_to_liq_pool, &self);

        //complete the transfer, remove stnear from the user (stnear was transferred to the LP & others)
        user_account.sub_st_near(st_near_to_sell, &self);
        //mint $META for the selling user
        let meta_to_seller = damp_multiplier(
            fee_in_st_near,
            self.stnear_sell_meta_mult_pct,
            self.est_meta_rewards_lu,
            self.max_meta_rewards_lu,
        );
        self.total_meta += meta_to_seller;
        // keep track of meta rewards for lu's
        self.est_meta_rewards_lu += meta_to_seller;
        user_account.realized_meta += meta_to_seller;

        //Save involved accounts
        self.internal_update_account(&self.treasury_account_id.clone(), &treasury_account);
        self.internal_update_account(&self.operator_account_id.clone(), &operator_account);
        self.internal_update_account(&DEVELOPERS_ACCOUNT_ID.into(), &developers_account);
        //Save nslp accounts
        self.internal_save_nslp_account(&nslp_account);

        //simplified user-flow
        //direct transfer to user (instead of leaving it in-contract as "available")
        let transfer_amount = user_account.take_from_available(near_to_receive, self);
        self.native_transfer_to_predecessor(transfer_amount);

        //Save user account
        self.internal_update_account(&account_id, &user_account);

        log!(
            "@{} liquid-unstaked {} stNEAR, got {} NEAR and {} $META",
            &account_id,
            st_near_to_sell,
            transfer_amount,
            meta_to_seller
        );
        event!(
            r#"{{"event":"LIQ.U","account_id":"{}","stnear":"{}","near":"{}"}}"#,
            &account_id,
            st_near_to_sell,
            transfer_amount
        );

        return LiquidUnstakeResult {
            near: transfer_amount.into(),
            fee: fee_in_st_near.into(),
            meta: meta_to_seller.into(),
        };
    }

    /// add liquidity - payable
    #[payable]
    pub fn nslp_add_liquidity(&mut self) -> u16 {
        // TODO: Since this method doesn't guard the resulting liquidity, is it possible to put it
        //    into a front-run/end-run sandwich to capitalize on the transaction?
        self.internal_deposit();
        return self.internal_nslp_add_liquidity(env::attached_deposit());
    }

    /// remove liquidity from liquidity pool
    //#[payable]
    pub fn nslp_remove_liquidity(&mut self, amount: U128String) -> RemoveLiquidityResult {
        self.assert_not_busy();
        //assert_one_yocto();

        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);
        let mut nslp_account = self.internal_get_nslp_account();

        //use this LP operation to realize meta pending rewards
        acc.nslp_realize_meta(&nslp_account, self);

        //how much does this user owns
        let valued_actual_shares = acc.valued_nslp_shares(self, &nslp_account);

        let mut to_remove = amount.0;
        let nslp_shares_to_burn: u128;
        // if the amount is close to user's total, remove user's total
        // to: a) do not leave less than ONE_MILLI_NEAR in the account, b) Allow 10 yoctos of rounding, e.g. remove(100) removes 99.999993 without panicking
        if is_close(to_remove, valued_actual_shares) {
            // allow for rounding simplification
            to_remove = valued_actual_shares;
            nslp_shares_to_burn = acc.nslp_shares; // close enough to all shares, burn-it all (avoid leaving "dust")
        } else {
            assert!(
                valued_actual_shares >= to_remove,
                "Not enough share value {} to remove the requested amount from the pool",
                valued_actual_shares
            );
            // Calculate the number of "nslp" shares that the account will burn based on the amount requested
            nslp_shares_to_burn = self.nslp_shares_from_amount(to_remove, &nslp_account);
        }

        assert!(nslp_shares_to_burn > 0);

        //register removed liquidity to compute rewards correctly
        acc.lp_meter.unstake(to_remove);

        //compute proportionals stNEAR/NEAR
        //1st: stNEAR how much stNEAR from the Liq-Pool represents the ratio: nslp_shares_to_burn relative to total nslp_shares
        let st_near_to_remove_from_pool = proportional(
            nslp_account.stake_shares,
            nslp_shares_to_burn,
            nslp_account.nslp_shares,
        );
        //2nd: NEAR, by difference
        let near_value_of_st_near = self.amount_from_stake_shares(st_near_to_remove_from_pool);
        assert!(
            to_remove >= near_value_of_st_near,
            "inconsistency NTR<STR+UTR"
        );
        let near_to_remove = to_remove - near_value_of_st_near;

        //update user account
        //remove first from stNEAR in the pool, proportional to shares being burned
        //NOTE: To simplify user-operations, the LIQ.POOL DO NOT carry "unstaked". The NSLP self-balances only by internal-clearing on `deposit_and_stake`
        acc.available += near_to_remove;
        acc.add_st_near(st_near_to_remove_from_pool, &self); //add stnear to user acc
        acc.nslp_shares -= nslp_shares_to_burn; //shares this user burns
                                                //update NSLP account
        nslp_account.available -= near_to_remove;
        nslp_account.sub_st_near(st_near_to_remove_from_pool, &self); //remove stnear from the pool
        nslp_account.nslp_shares -= nslp_shares_to_burn; //burn from total nslp shares

        //simplify user-flow
        //direct transfer to user (instead of leaving it in-contract as "available")
        let transfer_amount = acc.take_from_available(near_to_remove, self);
        self.native_transfer_to_predecessor(transfer_amount);

        //--SAVE ACCOUNTS
        self.internal_update_account(&account_id, &acc);
        self.internal_save_nslp_account(&nslp_account);

        event!(
            r#"{{"event":"REM.L","account_id":"{}","near":"{}","stnear":"{}"}}"#,
            account_id,
            transfer_amount,
            st_near_to_remove_from_pool
        );

        return RemoveLiquidityResult {
            near: transfer_amount.into(),
            st_near: st_near_to_remove_from_pool.into(),
        };
    }

    //------------------
    // REALIZE META
    //------------------
    /// massive convert $META from virtual to secure. IF multipliers are changed, virtual meta can decrease, this fn realizes current meta to not suffer loses
    /// for all accounts from index to index+limit
    pub fn realize_meta_massive(&mut self, from_index: u64, limit: u64) {
        for inx in
            from_index..std::cmp::min(from_index + limit, self.accounts.keys_as_vector().len())
        {
            let account_id = &self.accounts.keys_as_vector().get(inx).unwrap();
            if account_id == NSLP_INTERNAL_ACCOUNT {
                continue;
            }
            let mut acc = self.internal_get_account(&account_id);
            let prev_meta = acc.realized_meta;

            acc.stake_realize_meta(self);
            //get NSLP account
            let nslp_account = self.internal_get_nslp_account();
            //realize and mint meta from LP rewards
            acc.nslp_realize_meta(&nslp_account, self);
            if prev_meta != acc.realized_meta {
                self.internal_update_account(&account_id, &acc);
            }
        }
    }

    pub fn realize_meta(&mut self, account_id: String) {
        let mut acc = self.internal_get_account(&account_id);

        //realize and mint $META from staking rewards
        acc.stake_realize_meta(self);

        //get NSLP account
        let nslp_account = self.internal_get_nslp_account();
        //realize and mint meta from LP rewards
        acc.nslp_realize_meta(&nslp_account, self);

        self.internal_update_account(&account_id, &acc);
    }

    //------------------
    // HARVEST META
    //------------------
    #[payable]
    ///compute all $META rewards at this point and mint $META tokens in the meta-token NEP-141 contract for the user
    pub fn harvest_meta(&mut self) -> Promise {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);

        //realize and mint $META from staking rewards
        acc.stake_realize_meta(self);

        //get NSLP account
        let nslp_account = self.internal_get_nslp_account();
        //realize and mint meta from LP rewards
        acc.nslp_realize_meta(&nslp_account, self);

        // Note: we make `acc.realized_meta = 0` here and rollback the changes in
        //    `Self::after_minting_meta` in case the transfer fails.
        //    This is to not be vulnerable to the multi-call attack.
        //    If we don't, While `mint` is still pending, the attacker may call `harvest_meta`
        //    again and get `realized_meta` transferred multiple times.
        let to_mint = acc.realized_meta;
        //--SAVE ACCOUNT
        acc.realized_meta = 0;
        self.internal_update_account(&account_id, &acc);

        //schedule async to mint the $META-tokens for the user
        meta_token_mint::mint(
            account_id.clone(), // to whom
            to_mint.into(),     //how much meta
            // extra call args
            &self.meta_token_account_id,
            1, // 1 yocto hack
            gas::BASE_GAS,
        )
        .then(ext_self_owner::after_minting_meta(
            account_id,     // to whom
            to_mint.into(), // how much
            // extra call args
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::BASE_GAS,
        ))
    }
    //prev fn continues here
    #[private]
    pub fn after_minting_meta(&mut self, account_id: AccountId, to_mint: U128String) {
        if !is_promise_success() {
            //minting $META failed, rollback
            let mut acc = self.internal_get_account(&account_id);
            acc.realized_meta = to_mint.0;
            self.internal_update_account(&account_id, &acc);
        }
    }

    //---------------------------------------------------------------------------
    /// Sputnik DAO remote-upgrade receiver
    /// can be called by a remote-upgrade proposal
    ///
    #[cfg(target_arch = "wasm32")]
    pub fn upgrade(self) {
        assert!(env::predecessor_account_id() == self.owner_account_id);
        //input is code:<Vec<u8> on REGISTER 0
        //log!("bytes.length {}", code.unwrap().len());
        const GAS_FOR_UPGRADE: u64 = 20 * TGAS; //gas occupied by this fn
        const BLOCKCHAIN_INTERFACE_NOT_SET_ERR: &str = "Blockchain interface not set.";
        //after upgrade we call *pub fn migrate()* on the NEW CODE
        let current_id = env::current_account_id().into_bytes();
        let migrate_method_name = "migrate".as_bytes().to_vec();
        let attached_gas = env::prepaid_gas() - env::used_gas() - GAS_FOR_UPGRADE;
        unsafe {
            BLOCKCHAIN_INTERFACE.with(|b| {
                // Load input (new contract code) into register 0
                b.borrow()
                    .as_ref()
                    .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                    .input(0);

                //prepare self-call promise
                let promise_id = b
                    .borrow()
                    .as_ref()
                    .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                    .promise_batch_create(current_id.len() as _, current_id.as_ptr() as _);

                //1st action, deploy/upgrade code (takes code from register 0)
                b.borrow()
                    .as_ref()
                    .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                    .promise_batch_action_deploy_contract(promise_id, u64::MAX as _, 0);

                //2nd action, schedule a call to "migrate()".
                //Will execute on the **new code**
                b.borrow()
                    .as_ref()
                    .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                    .promise_batch_action_function_call(
                        promise_id,
                        migrate_method_name.len() as _,
                        migrate_method_name.as_ptr() as _,
                        0 as _,
                        0 as _,
                        0 as _,
                        attached_gas,
                    );
            });
        }
    }
}

//---------------
//TODO Unit tests.
//Note: Most tests are in /tests and are simulation-testing
//---------------
#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    //use std::convert::TryInto;

    use near_sdk::{testing_env, MockedBlockchain, VMContext}; //PromiseResult,

    mod unit_test_utils;
    use unit_test_utils::*;

    use super::*;

    //pub type AccountId = String;

    //const SALT: [u8; 3] = [1, 2, 3];

    fn basic_context() -> VMContext {
        get_context(
            system_account(),
            ntoy(TEST_INITIAL_BALANCE),
            0,
            to_ts(GENESIS_TIME_IN_DAYS),
            false,
        )
    }

    fn new_contract() -> MetaPool {
        MetaPool::new(
            owner_account(),
            treasury_account(),
            operator_account(),
            meta_token_account(),
        )
    }

    fn contract_only_setup() -> (VMContext, MetaPool) {
        let context = basic_context();
        testing_env!(context.clone());
        let contract = new_contract();
        return (context, contract);
    }

    /*
    #[test]
    fn test_internal_fee_curve() {
        let (_context, contract) = contract_only_setup();

        assert!( contract.internal_get_discount_basis_points(ntoy(10), ntoy(10)) == contract.nslp_max_discount_basis_points);

        assert!( contract.internal_get_discount_basis_points(contract.nslp_liquidity_target+ntoy(100), ntoy(100)) == contract.nslp_min_discount_basis_points);

        println!("target {} min {} max {} ", yton(contract.nslp_liquidity_target), contract.nslp_min_discount_basis_points, contract.nslp_max_discount_basis_points);
        for n in 0..12 {
            let liquidity = (contract.nslp_liquidity_target + ntoy(2000)).saturating_sub(ntoy(n*1000));
            let sell = ntoy(1000);
            let low = contract.internal_get_discount_basis_points(liquidity, sell);
            println!("liquidity {}, sell {}, fee {}%", yton(liquidity), yton(sell), low as f64/100.0);
        }

        //assert!( low  > contract.nslp_min_discount_basis_points);
        assert!( contract.internal_get_discount_basis_points(ntoy(9_000), ntoy(1_000)) == 56);
        assert!( contract.internal_get_discount_basis_points(ntoy(6_000), ntoy(1_000)) == 103);
        assert!( contract.internal_get_discount_basis_points(ntoy(2_000), ntoy(1_000)) == 165);
        //assert!(false);

    }
    */

    #[test]
    fn test_internal_get_near_amount_sell_stnear() {
        let (_context, contract) = contract_only_setup();
        let lp_balance_y: u128 = ntoy(500_000);
        let sell_stnear_y: u128 = ntoy(120);

        let discount_bp: u16 =
            contract.internal_get_discount_basis_points(lp_balance_y, sell_stnear_y);

        let near_amount_received_y =
            contract.internal_get_near_amount_sell_stnear(lp_balance_y, sell_stnear_y);

        let st_near_price = contract.amount_from_stake_shares(ONE_NEAR);
        let sold_value_near = st_near_price * sell_stnear_y;

        assert!(near_amount_received_y <= sold_value_near); //we were charged a fee

        let discounted_y = sold_value_near - near_amount_received_y;
        let _discounted_display_n = ytof(discounted_y);
        let _sell_stnear_display_n = ytof(sell_stnear_y);
        assert!(discounted_y == apply_pct(discount_bp, sold_value_near));
        assert!(near_amount_received_y == sold_value_near - discounted_y);
    }

    #[test]
    #[ignore]
    #[should_panic(expected = "Can only be called by the owner")]
    fn test_call_by_non_owner() {
        let (mut context, mut contract) = contract_only_setup();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR);
        context.predecessor_account_id = non_owner();
        context.signer_account_id = non_owner();
        testing_env!(context.clone());

        contract.set_operator_account_id(AccountId::from("staking_pool"));
    }

    #[test]
    fn test_rewards_meter() {
        let mut rm = RewardMeter::default();
        rm.stake(100);
        assert_eq!(rm.compute_rewards(105, 500, 1000), 5);

        rm.unstake(105);
        assert_eq!(rm.compute_rewards(0, 500, 1000), 0);

        rm.stake(10);
        assert_eq!(rm.compute_rewards(11, 500, 1000), 6);
    }
}
