//! A smart contract that allows diversified staking, SKASH and G-SKASH farming
//! this contract include parts of core-contracts/lockup-contract & core-contracts/staking-pool

/********************************/
/* CONTRACT Self Identification */
/********************************/
// [NEP-129](https://github.com/nearprotocol/NEPs/pull/129)
// see also pub fn get_contract_info
const CONTRACT_NAME: &str = "diversifying staking pool";
const CONTRACT_VERSION: &str = "0.1.0";
const DEFAULT_WEB_APP_URL: &str = "http://div-pool.narwallets.com";
const DEFAULT_AUDITOR_ACCOUNT_ID: &str = "auditors.near";

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::Base58PublicKey;
use near_sdk::{collections::UnorderedMap, env, ext_contract, near_bindgen, AccountId};

pub use crate::internal::*;
pub use crate::owner::*;
pub use crate::types::*;
pub use crate::utils::*;

pub mod gas;
pub mod types;
pub mod utils;

pub mod internal;
pub mod owner;

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc = near_sdk::wee_alloc::WeeAlloc::INIT;

const NSLP_INTERNAL_ACCOUNT: &str = "..NSLP..";

#[ext_contract(ext_staking_pool)]
pub trait ExtStakingPool {
    fn get_account_staked_balance(&self, account_id: AccountId) -> U128String;

    fn get_account_unstaked_balance(&self, account_id: AccountId) -> U128String;

    fn get_account_total_balance(&self, account_id: AccountId) -> U128String;

    fn deposit(&mut self);

    fn deposit_and_stake(&mut self);

    fn withdraw(&mut self, amount: U128String);

    fn stake(&mut self, amount: U128String);

    fn unstake(&mut self, amount: U128String);

    fn unstake_all(&mut self);
}

#[ext_contract(ext_self_owner)]
pub trait ExtDivPoolContractOwner {
    fn on_staking_pool_deposit(&mut self, amount: U128String) -> bool;

    fn on_staking_pool_withdraw(&mut self, sp_inx: usize) -> bool;

    fn on_staking_pool_stake_maybe_deposit(
        &mut self,
        sp_inx: usize,
        amount: u128,
        included_deposit: bool,
    ) -> bool;

    fn on_staking_pool_unstake(&mut self, sp_inx: usize, amount: u128) -> bool;

    //fn on_staking_pool_unstake_all(&mut self) -> bool;

    fn on_get_result_from_transfer_poll(&mut self, #[callback] poll_result: PollResult) -> bool;

    fn on_get_sp_total_balance(&mut self, sp_inx: usize, #[callback] total_balance: U128String);
}

// -----------------
// Reward meter utility
// -----------------
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq)]
pub struct RewardMeter {
    ///added with staking
    ///subtracted on unstaking. WARN: Since unstaking can inlude rewards, delta_staked *CAN BECOME NEGATIVE*
    pub delta_staked: i128,
    /// (pct: 100 => x1, 200 => x2)
    pub last_multiplier_pct: u16,
}

impl Default for RewardMeter {
    fn default() -> Self {
        Self {
            delta_staked: 0,
            last_multiplier_pct: 100,
        }
    }
}

impl RewardMeter {
    /// compute rewards received (extra after stake/unstake)
    /// multiplied by last_multiplier_pct%
    pub fn compute_rewards(&self, valued_shares: u128) -> u128 {
        if self.delta_staked > 0 && valued_shares == (self.delta_staked as u128) {
            return 0; //fast exit
        }
        assert!(valued_shares < ((i128::MAX - self.delta_staked) as u128), "TB");
        assert!(
            self.delta_staked < 0 || valued_shares >= (self.delta_staked as u128),
            "valued_shares:{} .LT. self.delta_staked:{}",valued_shares,self.delta_staked
        );
        // valued_shares - self.delta_staked => true rewards
        return (
            U256::from( (valued_shares as i128) - self.delta_staked )
            * U256::from(self.last_multiplier_pct) / U256::from(100)
        ).as_u128();
    }
    ///register a stake (to be able to compute rewards later)
    pub fn stake(&mut self, value: u128) {
        assert!(value < (i128::MAX as u128));
        self.delta_staked += value as i128;
    }
    ///register a unstake (to be able to compute rewards later)
    pub fn unstake(&mut self, value: u128) {
        assert!(value < (i128::MAX as u128));
        self.delta_staked -= value as i128;
    }
    ///realize rewards
    /// compute rewards received (extra after stake/unstake) multiplied by last_multiplier_pct%
    /// adds to self.realized
    /// then reset the meter to zero
    /// and maybe update the multiplier
    pub fn realize(&mut self, valued_shares: u128, new_multiplier_pct: u16) -> u128 {
        let result = self.compute_rewards(valued_shares);
        self.delta_staked = valued_shares as i128; // reset meter to Zero
        self.last_multiplier_pct = new_multiplier_pct; //maybe changed, start aplying new multiplier
        return result;
    }
}

// -----------------
// User Account Data
// -----------------
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq)]
pub struct Account {
    /// This amount increments with deposits and decrements with for_staking
    /// increments with complete_unstake and decrements with user withdrawals from the contract
    /// withdrawals from the pools can include rewards
    /// since statking is delayed and in batches it only eventually matches env::balance()
    /// total = available + staked + unstaked
    pub available: u128,

    /// The amount of shares of the total staked balance in the pool(s) this user owns.
    /// Before someone stakes share-price is computed and shares are "sold" to the user so he only owns what he's staking and no rewards yet
    /// When a user reequest a transfer to other user, staked & shares from the origin are moved to staked & shares of the destination
    /// The share_price can be computed as total_for_staking/total_stake_shares
    /// shares * share_price = SKASHs
    stake_shares: u128,

    /// Incremented when the user asks for unstaking. The amount of unstaked near in the pools
    pub unstaked: u128,

    /// The epoch height when the unstaked was requested
    /// The fund will be locked for -AT LEAST- NUM_EPOCHS_TO_UNLOCK epochs
    /// unlock epoch = unstaked_requested_epoch_height + NUM_EPOCHS_TO_UNLOCK
    pub unstaked_requested_epoch_height: EpochHeight,

    //-- G-SKASH
    ///realized G-SKASH, can be used to transfer G-SKASH from one user to another
    // Total G-SKASH = realized_g_skash + staking_meter.mul_rewards(valued_stake_shares) + lp_meter.mul_rewards(valued_lp_shares)
    // Every time the user operates on STAKE/UNSTAKE: we realize g-skash: realized_g_skash += staking_meter.mul_rewards(valued_staked_shares)
    // Every time the user operates on ADD.LIQ/REM.LIQ.: we realize g-skash: realized_g_skash += lp_meter.mul_rewards(valued_lp_shares)
    // if the user calls farm_g_skash() we perform both
    pub realized_g_skash: u128,
    ///Staking rewards meter (to mint skash for the user)
    pub staking_meter: RewardMeter,
    ///LP fee gains meter (to mint g-skash for the user)
    pub lp_meter: RewardMeter,

    //-- STATISTICAL DATA --
    // User's statistical data
    // This is the user-cotrolled staking rewards meter, it works as a car's "trip meter". The user can reset them to zero.
    // to compute trip_rewards we start from current_skash, undo unstakes, undo stakes and finally subtract trip_start_skash
    // trip_rewards = current_skash + trip_accum_unstakes - trip_accum_stakes - trip_start_skash;
    /// trip_start: (timpestamp in miliseconds) this field is set at account creation, so it will start metering rewards
    pub trip_start: Timestamp,

    /// How much skashs the user had at "trip_start".
    pub trip_start_skash: u128,
    // how much skahs the staked since trip start. always incremented
    pub trip_accum_stakes: u128,
    // how much the user unstaked since trip start. always incremented
    pub trip_accum_unstakes: u128,

    ///NS liquidity pool shares, if the user is a liquidity provider
    pub nslp_shares: u128,
}

/// User account on this contract
impl Default for Account {
    fn default() -> Self {
        Self {
            available: 0,
            stake_shares: 0,
            unstaked: 0,
            unstaked_requested_epoch_height: 0,
            //g-skash & reward-meters
            realized_g_skash: 0,
            staking_meter: RewardMeter::default(),
            lp_meter: RewardMeter::default(),
            //trip-meter fields
            trip_start: env::block_timestamp() / 1_000_000, //converted from nanoseconds to miliseconds
            trip_start_skash: 0,
            trip_accum_stakes: 0,
            trip_accum_unstakes: 0,
            //NS liquidity pool
            nslp_shares: 0,
        }
    }
}
impl Account {
    /// when the account.is_empty() it will be removed
    fn is_empty(&self) -> bool {
        return self.available == 0
            && self.unstaked == 0
            && self.stake_shares == 0
            && self.nslp_shares == 0
            && self.realized_g_skash == 0;
    }

    #[inline]
    fn valued_nslp_shares(&self, main: &DiversifiedPool, nslp_account: &Account) -> u128 { main.amount_from_nslp_shares(self.nslp_shares, &nslp_account) }

    /// return realized g_skash plus pending rewards
    fn total_g_skash(&self, main: &DiversifiedPool) -> u128 {
        let valued_stake_shares = main.amount_from_stake_shares(self.stake_shares);
        let nslp_account = main.internal_get_nslp_account();
        let valued_lp_shares = self.valued_nslp_shares(main, &nslp_account);
        return self.realized_g_skash
            + self.staking_meter.compute_rewards(valued_stake_shares)
            + self.lp_meter.compute_rewards(valued_lp_shares);
    }


    //---------------------------------
    fn stake_realize_g_skash(&mut self, main:&mut DiversifiedPool) {
        //realize g-skash pending rewards on LP operation
        let valued_actual_shares = main.amount_from_stake_shares(self.stake_shares);
        let pending_g_skash = self.staking_meter.realize(valued_actual_shares, main.staker_g_skash_mult_pct);
        self.realized_g_skash += pending_g_skash;
        main.total_g_skash += pending_g_skash;
    }

    fn nslp_realize_g_skash(&mut self, nslp_account:&Account, main:&mut DiversifiedPool)  {
        //realize g-skash pending rewards on LP operation
        let valued_actual_shares = self.valued_nslp_shares(main, &nslp_account);
        let pending_g_skash = self.lp_meter.realize(valued_actual_shares, main.lp_provider_g_skash_mult_pct);
        self.realized_g_skash += pending_g_skash;
        main.total_g_skash += pending_g_skash;
    }

    //----------------
    fn add_stake_shares(&mut self, num_shares:u128, skash:u128){
        self.stake_shares += num_shares;
        //to buy skash is stake
        self.trip_accum_stakes += skash;
        self.staking_meter.stake(skash);
    }
    fn remove_stake_shares(&mut self, num_shares:u128, skash:u128){
        assert!(self.stake_shares>num_shares,"RSS-NES");
        self.stake_shares -= num_shares;
        //to sell skash is to unstake
        self.trip_accum_unstakes += skash;
        self.staking_meter.unstake(skash);
    }

}

//-------------------------
//--  STAKING POOLS LIST --
//-------------------------
/// items in the Vec of staking pools
#[derive(Default)]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakingPoolInfo {
    pub account_id: AccountId,

    //how much of the meta-pool must be staked in this pool
    //0=> do not stake, only unstake
    //100 => 1% , 250=>2.5%, etc. -- max: 10000=>100%
    pub weight_basis_points: u16,

    //if we've made an async call to this pool
    pub busy_lock: bool,

    //total staked here
    pub staked: u128,

    //total unstaked in this pool
    pub unstaked: u128,
    //set when the unstake command is passed to the pool
    //waiting period is until env::EpochHeight == unstaked_requested_epoch_height+NUM_EPOCHS_TO_UNLOCK
    //We might have to block users from unstaking if all the pools are in a waiting period
    pub unstk_req_epoch_height: EpochHeight, // = env::epoch_height() + NUM_EPOCHS_TO_UNLOCK

    //EpochHeight where we asked the sp what were our staking rewards
    pub last_asked_rewards_epoch_height: EpochHeight,
}

impl StakingPoolInfo {
    pub fn is_empty(&self) -> bool {
        return self.busy_lock == false
            && self.weight_basis_points == 0
            && self.staked == 0
            && self.unstaked == 0
    }
    pub fn new(account_id:AccountId, weight_basis_points: u16) -> Self {
        return Self {
            account_id,
            weight_basis_points,
            busy_lock: false,
            staked:0,
            unstaked:0,
            unstk_req_epoch_height:0,
            last_asked_rewards_epoch_height:0
        }
    }
}

//------------------------
//  Main Contract State --
//------------------------
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct DiversifiedPool {
    /// Owner's account ID (it will be a DAO on phase II)
    pub owner_account_id: String,

    /// if you're holding skash there's a min balance you must mantain to backup storage usage
    /// can be adjusted down by keeping the required NEAR in the developers or operator account
    pub min_account_balance: u128,

    // [NEP-129](https://github.com/nearprotocol/NEPs/pull/129)
    pub web_app_url: String, 
    pub auditor_account_id: String,

    /// Operator account ID (who's in charge to call distribute() on a periodic basis)
    pub operator_account_id: String,
    /// operator_rewards_fee_basis_points. (0.2% default) 100 basis point => 1%. E.g.: owner_fee_basis_points=30 => 0.3% owner's fee
    pub operator_rewards_fee_basis_points: u16,
    /// owner's cut on SHKASH Sell fee (3% default)
    pub operator_swap_cut_basis_points: u16,
    /// Treasury account ID (it will be controlled by a DAO on phase II)
    pub treasury_account_id: String,
    /// treasury cut on SHKASH Sell cut (25% default)
    pub treasury_swap_cut_basis_points: u16,

    /// This amount increments with deposits and decrements when users stake
    /// increments with complete_unstake and decrements with user withdrawals from the contract
    /// withdrawals from the pools can include rewards
    /// since staking is delayed and in batches it only eventually matches env::balance()
    pub total_available: u128,

    /// The total amount of tokens selected for staking by the users
    /// not necessarily what's actually staked since staking can is done in batches
    /// Share price is computed using this number. share_price = total_for_staking/total_shares
    pub total_for_staking: u128,

    /// The total amount of tokens actually staked (the tokens are in the staking pools)
    /// During distribute(), If !staking_paused && total_for_staking<total_actually_staked, then the difference gets staked in 100kN batches
    pub total_actually_staked: u128,

    // how many "shares" were minted. Everytime someone "stakes" he "buys pool shares" with the staked amount
    // the share price is computed so if he "sells" the shares on that moment he recovers the same near amount
    // staking produces rewards, so share_price = total_for_staking/total_shares
    // when someone "unstakes" she "burns" X shares at current price to recoup Y near
    pub total_stake_shares: u128,

    /// total g-skash minted
    pub total_g_skash: u128,

    /// The total amount of tokens selected for unstaking by the users
    /// not necessarily what's actually unstaked since unstaking is done in batches
    /// If a user ask unstaking 100: total_for_unstaking+=100, total_for_staking-=100, total_stake_shares-=share_amount
    pub total_for_unstaking: u128,

    /// The total amount of tokens actually unstaked (the tokens are in the staking pools)
    /// During distribute(), If !staking_paused && total_for_unstaking<total_actually_unstaked, then the difference gets unstaked in 100kN batches
    pub total_actually_unstaked: u128,

    /// The total amount of tokens actually unstaked AND retrieved from the pools (the tokens are here)
    /// During distribute(), If sp.pending_withdrawal && sp.epoch_for_withdraw == env::epoch_height then all funds are retrieved from the sp
    /// When the funds are actually withdraw by the users, total_actually_unstaked is decremented
    pub total_actually_unstaked_and_retrieved: u128,

    /// the staking pools will add rewards to the staked amount on each epoch
    /// here we store the accumulatred amount only for stats purposes. This amount can only grow
    pub accumulated_staked_rewards: u128,

    /// no auto-staking. true while changing staking pools
    pub staking_paused: bool,

    //user's accounts
    pub accounts: UnorderedMap<String, Account>,

    //list of pools to diversify in
    pub staking_pools: Vec<StakingPoolInfo>,

    //last epoch we checkd for unstaking-delayed funds to withdraw
    pub withdraw_unstaked_last_epoch_checked: EpochHeight,

    //The next 3 values define the Liq.Provider fee curve
    // NEAR/SKASH Liquidity pool fee curve params
    // We assume this pool is always UNBALANCED, there should be more SKASH than NEAR 99% of the time
    ///NEAR/SKASH Liquidity pool target
    pub nslp_near_target: u128,
    ///NEAR/SKASH Liquidity pool max fee
    pub nslp_max_discount_basis_points: u16, //10%
    ///NEAR/SKASH Liquidity pool min fee
    pub nslp_min_discount_basis_points: u16, //0.1%

    //The next 3 values define g-skash rewards multiplers %. (100 => 1x, 200 => 2x, ...)
    ///for each SKASH paid staking reward, reward SKASH holders with g-SKASH. default:5x. reward G-SKASH = rewards * mult_pct / 100
    pub staker_g_skash_mult_pct: u16,
    ///for each SKASH paid as discount, reward SKASH sellers with g-SKASH. default:1x. reward G-SKASH = discounted * mult_pct / 100
    pub skash_sell_g_skash_mult_pct: u16,
    ///for each SKASH paid as discount, reward SKASH sellers with g-SKASH. default:20x. reward G-SKASH = fee * mult_pct / 100
    pub lp_provider_g_skash_mult_pct: u16,
}

impl Default for DiversifiedPool {
    fn default() -> Self {
        env::panic(b"The contract is not initialized.");
    }
}

#[near_bindgen]
impl DiversifiedPool {
    /* NOTE
    This contract implements several traits

    1. deposit-trait [NEP-xxx]: this contract implements: deposit, get_account_total_balance, get_account_available_balance, withdraw, withdraw_all
       A [NEP-xxx] contract creates an account on deposit and allows you to withdraw later under certain conditions. Deletes the account on withdraw_all

    2. staking-pool [NEP-xxx]: this contract must be perceived as a staking-pool for the lockup-contract, wallets, and users.
        This means implmenting: ping, deposit, deposit_and_stake, withdraw_all, withdraw, stake_all, stake, unstake_all, unstake
        and view methods: get_account_unstaked_balance, get_account_staked_balance, get_account_total_balance, is_account_unstaked_balance_available,
            get_total_staked_balance, get_owner_id, get_reward_fee_fraction, is_staking_paused, get_staking_key, get_account,
            get_number_of_accounts, get_accounts.

    3. diversified-staking: these are the extensions to the standard staking pool (buy/sell skash, finish_unstake)

    4. multitoken (TODO) [NEP-xxx]: this contract implements: deposit(tok), get_token_balance(tok), withdraw_token(tok), tranfer_token(tok), transfer_token_to_contract(tok)
       A [NEP-xxx] manages multiple tokens

    */

    /// Requires 25 TGas (1 * BASE_GAS)
    ///
    /// Initializes DiversifiedPool contract.
    /// - `owner_account_id` - the account ID of the owner.  Only this account can call owner's methods on this contract.
    #[init]
    pub fn new(
        owner_account_id: AccountId,
        treasury_account_id: AccountId,
        operator_account_id: AccountId,
    ) -> Self {
        assert!(!env::state_exists(), "The contract is already initialized");

        return Self {
            owner_account_id,
            operator_account_id,
            treasury_account_id,
            min_account_balance: ONE_NEAR,
            web_app_url: DEFAULT_WEB_APP_URL.into(),
            auditor_account_id: DEFAULT_AUDITOR_ACCOUNT_ID.into(),
            operator_rewards_fee_basis_points: DEFAULT_OPERATOR_REWARDS_FEE_BASIS_POINTS,
            operator_swap_cut_basis_points: DEFAULT_OPERATOR_SWAP_CUT_BASIS_POINTS,
            treasury_swap_cut_basis_points: DEFAULT_TREASURY_SWAP_CUT_BASIS_POINTS,
            staking_paused: true, //no auto-staking. on while there's no staking pool defined
            total_available: 0,
            total_for_staking: 0,
            total_for_unstaking: 0,
            total_actually_staked: 0, //amount actually sent to the staking_pool and staked
            total_actually_unstaked: 0, // tracks unstaked amount from the staking_pool (toekns are in the pool)
            total_actually_unstaked_and_retrieved: 0, // tracks unstaked AND retrieved amount (tokens are here)
            accumulated_staked_rewards: 0,
            total_stake_shares: 0,
            total_g_skash: 0,
            accounts: UnorderedMap::new("A".into()),
            nslp_near_target: ONE_NEAR * 1_000_000,
            nslp_max_discount_basis_points: 1000, //10%
            nslp_min_discount_basis_points: 50,   //0.5%
            ///for each SKASH paid as discount, reward SKASH sellers with g-SKASH. default:1x. reward G-SKASH = discounted * mult_pct / 100
            skash_sell_g_skash_mult_pct: 100,
            ///for each SKASH paid staking reward, reward SKASH holders with g-SKASH. default:5x. reward G-SKASH = rewards * mult_pct / 100
            staker_g_skash_mult_pct: 500,
            ///for each SKASH paid as discount, reward SKASH sellers with g-SKASH. default:20x. reward G-SKASH = fee * mult_pct / 100
            lp_provider_g_skash_mult_pct: 2000,

            staking_pools: Vec::new(),
            withdraw_unstaked_last_epoch_checked:0,
        };
    }

    //pub fn set_min_balance(&mut self)

    //------------------------------------
    // deposit trait & staking-pool trait
    //------------------------------------

    /// staking-pool's ping redirects to diversified-pool's distribute, Does a bit of work
    pub fn ping(&mut self) {
        self.distribute();
    }

    /// Deposits the attached amount into the inner account of the predecessor.
    #[payable]
    pub fn deposit(&mut self) {
        self.internal_deposit();
    }

    /// Withdraws from the available balance
    pub fn withdraw(&mut self, amount: U128String) {
        self.internal_withdraw(amount.into());
    }

    /// Withdraws ALL from the "available" balance
    pub fn withdraw_all(&mut self) {
        let account_id = env::predecessor_account_id();
        let account = self.internal_get_account(&account_id);
        self.internal_withdraw(account.available);
    }

    /// Deposits the attached amount into the inner account of the predecessor and stakes it.
    #[payable]
    pub fn deposit_and_stake(&mut self) {
        self.internal_deposit();
        self.internal_stake(env::attached_deposit());
    }

    /// Stakes all available unstaked balance from the inner account of the predecessor.
    /// staking-pool "unstaked" is equivalent to diversified-pool "available", but here
    /// we keep the staking-pool logic because we're implementing the staking-pool trait
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
        self.inner_unstake(amount);
    }

    /// Unstakes the given amount from the inner account of the predecessor.
    /// The inner account should have enough staked balance.
    /// The new total unstaked balance will be available for withdrawal in four epochs.
    pub fn unstake(&mut self, amount: U128String) {
        self.inner_unstake(amount.0);
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
        return (acc.available + self.amount_from_stake_shares(acc.stake_shares)+ acc.unstaked).into();
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
            numerator: (self.operator_rewards_fee_basis_points + DEVELOPERS_REWARDS_FEE_BASIS_POINTS)
                .into(),
            denominator: 10_000,
        };
    }

    /// Returns the staking public key
    pub fn get_staking_key(&self) -> Base58PublicKey {
        panic!("no specific staking key for the div-pool");
    }

    /// Returns true if the staking is paused
    pub fn is_staking_paused(&self) -> bool {
        return self.staking_paused;
    }

    /// to implement the Staking-pool inteface, get_account returns the same as the staking-pool returns
    /// full account info can be obtained by calling: pub fn get_account_info(&self, account_id: AccountId) -> GetAccountInfoResult
    /// Returns human readable representation of the account for the given account ID.
    //warning: self.get_account is public and gets HumanReadableAccount .- do not confuse with self.internal_get_account
    pub fn get_account(&self, account_id: AccountId) -> HumanReadableAccount {
        let account = self.internal_get_account(&account_id);
        return HumanReadableAccount {
            account_id,
            unstaked_balance: account.unstaked.into(),
            staked_balance: self.amount_from_stake_shares(account.stake_shares).into(),
            can_withdraw: env::epoch_height()
                >= account.unstaked_requested_epoch_height + NUM_EPOCHS_TO_UNLOCK,
        };
    }

    /// Returns the number of accounts that have positive balance on this staking pool.
    pub fn get_number_of_accounts(&self) -> u64 {
        return self.accounts.len();
    }

    /// Returns the list of accounts
    //warning: self.get_accounts is public and gets HumanReadableAccount .- do not confuse with self.internal_get_account
    pub fn get_accounts(&self, from_index: u64, limit: u64) -> Vec<HumanReadableAccount> {
        let keys = self.accounts.keys_as_vector();
        return (from_index..std::cmp::min(from_index + limit, keys.len()))
            .map(|index| self.get_account(keys.get(index).unwrap()))
            .collect();
    }

    //----------------------------------
    //----------------------------------
    // DIVERISIFYING-STAKING-POOL trait
    //----------------------------------
    //----------------------------------

    /// user method
    /// completes unstake action by moving from retreieved_from_the_pools to available
    pub fn finish_unstaking(&mut self) {
        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);
        let amount = account.unstaked;
        assert!(amount > 0, "No unstaked balance");
        let epoch = env::epoch_height();
        let epochs_to_wait = account.unstaked_requested_epoch_height + NUM_EPOCHS_TO_UNLOCK - epoch;
        if epoch < account.unstaked_requested_epoch_height + NUM_EPOCHS_TO_UNLOCK {
            panic!(format!("The unstaked balance is not yet available due to unstaking delay. You need to wait {} epochs", epochs_to_wait));
        }

        //async: try to do one of the pending withdrawals
        self.internal_async_withdraw_from_a_pool();

        if self.total_actually_unstaked_and_retrieved < amount {
            panic!("Please wait one more hour until the funds are retrieved from the pools");
        }

        assert!(self.total_for_unstaking >= amount);

        //used retrieved funds
        self.total_actually_unstaked_and_retrieved -= amount;
        // moves from total_for_unstaking to total_available
        self.total_for_unstaking -= amount;
        self.total_available += amount;
        // in the account, moves from unstaked to available
        account.unstaked -= amount;
        account.available += amount;
        self.internal_save_account(&account_id, &account);

        // env::log(
        //     format!(
        //         "@{} withdrawing {}. New unstaked balance is {}",
        //         account_id, amount, account.unstaked
        //     )
        //     .as_bytes(),
        // );
    }

    /// buy_skash_stake. Identical to stake, migth change in the future
    pub fn buy_skash_stake(&mut self, amount: U128String) {
        self.internal_stake(amount.0);
    }

    //---------------------------
    // NSLP Methods
    //---------------------------

    /// user method - NEAR/SKASH SWAP functions
    /// return how much NEAR you can get by selling x SKASH
    pub fn get_near_amount_sell_skash(&self, skash_to_sell: U128String) -> U128String {
        let lp_account = self.internal_get_nslp_account();
        return self.internal_get_near_amount_sell_skash(lp_account.available, skash_to_sell.0).into();
    }

    /// NEAR/SKASH Liquidity Pool
    /// computes the discount_basis_points for NEAR/SKASH Swap based on NSLP Balance
    /// If you want to sell x SKASH
    pub fn nslp_get_discount_basis_points(&self, skash_to_sell: U128String) -> u16 {
        let lp_account = self.internal_get_nslp_account();
        return self.internal_get_discount_basis_points(lp_account.available, skash_to_sell.0);
    }

    /// user method
    /// Sells-skash at discount in the NLSP
    /// returns near received
    pub fn sell_skash(
        &mut self,
        skash_to_sell: U128String,
        min_expected_near: U128String,
    ) -> U128String {
        let account_id = env::predecessor_account_id();
        let mut user_account = self.internal_get_account(&account_id);

        let skash_owned = self.amount_from_stake_shares(user_account.stake_shares);
        assert!(
            skash_owned >= skash_to_sell.0,
            "Not enough skash in your account"
        );
        let mut lp_account = self.internal_get_nslp_account();
        let near_to_receive =
            self.internal_get_near_amount_sell_skash(lp_account.available, skash_to_sell.0);
        assert!(
            near_to_receive >= min_expected_near.0,
            "Price changed, your min results requirements not satisfied. Try again"
        );
        assert!(
            lp_account.available >= near_to_receive,
            "lp_account.available < near_to_receive"
        );

        let stake_shares_sell = self.stake_shares_from_amount(skash_to_sell.0);
        assert!(
            user_account.stake_shares >= stake_shares_sell,
            "account.stake_shares < stake_shares_sell"
        );

        //swap shares(SKASH)<->NEAR between user account and lp_account
        lp_account.available -= near_to_receive;
        user_account.available += near_to_receive;

        user_account.remove_stake_shares(stake_shares_sell, skash_to_sell.0);

        //the fee is the difference between skash sold and near received
        assert!(near_to_receive < skash_to_sell.0);
        let fee_in_skash = skash_to_sell.0 - near_to_receive;
        // compute how many shares the swap fee represent
        let fee_in_shares = self.stake_shares_from_amount(fee_in_skash);

        // involved accounts
        let mut treasury_account = self.internal_get_account(&self.treasury_account_id);
        let mut operator_account = self.internal_get_account(&self.operator_account_id);
        let mut developers_account = self.internal_get_account(&DEVELOPERS_ACCOUNT_ID.into());

        // The treasury cut in skash-shares (25% by default)
        let treasury_stake_shares_cut = apply_pct(self.treasury_swap_cut_basis_points,fee_in_shares);
        let treasury_skash_cut = apply_pct(self.treasury_swap_cut_basis_points,fee_in_skash);
        treasury_account.add_stake_shares(treasury_stake_shares_cut,treasury_skash_cut);
        
        // The cut that the contract owner (operator) takes. (3% of 1% normally)
        let operator_stake_shares_cut = apply_pct( self.operator_swap_cut_basis_points,fee_in_shares);
        let operator_skash_cut = apply_pct( self.operator_swap_cut_basis_points, fee_in_skash);
        operator_account.add_stake_shares(operator_stake_shares_cut,operator_skash_cut);

        // The cut that the developers take. (2% of 1% normally)
        let developers_stake_shares_cut = apply_pct(DEVELOPERS_SWAP_CUT_BASIS_POINTS, fee_in_shares);
        let developers_skash_cut = apply_pct(DEVELOPERS_SWAP_CUT_BASIS_POINTS, fee_in_skash);
        developers_account.add_stake_shares(developers_stake_shares_cut,developers_skash_cut);

        // all the realized g-skash from non-liq.provider cuts (30%), send to operator & developers
        let skash_non_lp_cut = treasury_skash_cut+operator_skash_cut+developers_skash_cut;
        let g_skash_from_operation = apply_multiplier(skash_non_lp_cut, self.lp_provider_g_skash_mult_pct);
        self.total_g_skash += g_skash_from_operation;
        operator_account.realized_g_skash += g_skash_from_operation/2;
        developers_account.realized_g_skash += g_skash_from_operation/2;

        //The rest of the skash-fee (70%) go into the LP increasing share value for all LP providers.
        //Adding value to the pool via adding more skash than the near removed, will be counted as rewards for the nslp_meter, 
        // so g-skash for LP providers will be created. G-skash for LP providers are realized during add_liquidit(), remove_liquidity() or by calling harvest_g_skash_from_lp()
        assert!(stake_shares_sell > treasury_stake_shares_cut + developers_stake_shares_cut + operator_stake_shares_cut);
        lp_account.add_stake_shares( 
            fee_in_shares - (treasury_stake_shares_cut + operator_stake_shares_cut + developers_stake_shares_cut),
            fee_in_skash - (treasury_skash_cut + operator_skash_cut + developers_skash_cut ));

        //Save involved accounts
        self.internal_save_account(&self.treasury_account_id.clone(), &treasury_account);
        self.internal_save_account(&self.operator_account_id.clone(), &operator_account);
        self.internal_save_account(&DEVELOPERS_ACCOUNT_ID.into(), &developers_account);
        //Save user and nslp accounts
        self.internal_save_account(&NSLP_INTERNAL_ACCOUNT.into(), &lp_account);
        self.internal_save_account(&account_id, &user_account);

        env::log(
            format!(
                "@{} sold {} SKASH for {} NEAR",
                account_id, skash_to_sell.0, near_to_receive
            )
            .as_bytes(),
        );

        return near_to_receive.into();
    }


    /// add liquidity from deposited funds
    pub fn nslp_add_liquidity(&mut self, amount: U128String) {
        assert_min_amount(amount.0);

        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);

        assert!(
            acc.available >= amount.0,
            "Not enough available balance to add the requested amount to the NSLP"
        );

        //get NSLP account
        let mut nslp_account = self.internal_get_nslp_account();

        //use this LP operation to realize g-skash pending rewards (same as nslp_harvest_g_skash)
        acc.nslp_realize_g_skash(&nslp_account, self);

        // Calculate the number of "nslp" shares that the account will receive for adding the given amount of near liquidity
        let num_shares = self.nslp_shares_from_amount(amount.0, &nslp_account);
        assert!(num_shares > 0);

        //register added liquidity to compute rewards correctly
        acc.lp_meter.stake(amount.0);

        //update user account
        acc.available -= amount.0;
        acc.nslp_shares += num_shares;
        //update NSLP account
        nslp_account.available += amount.0;
        nslp_account.nslp_shares += num_shares; //total nslp shares

        //--SAVE ACCOUNTS
        self.internal_save_account(&account_id, &acc);
        self.internal_save_account(&NSLP_INTERNAL_ACCOUNT.into(), &nslp_account);
    }

    /// remove liquidity from deposited funds
    pub fn nslp_remove_liquidity(&mut self, amount: U128String) {
        
        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);
        let mut nslp_account = self.internal_get_nslp_account();

        //use this LP operation to realize g-skash pending rewards (same as nslp_harvest_g_skash)
        acc.nslp_realize_g_skash(&nslp_account, self);

        //how much does this user owns
        let valued_actual_shares = acc.valued_nslp_shares(self, &nslp_account);

        //register removed liquidity to compute rewards correctly
        acc.lp_meter.unstake(amount.0);

        let mut to_remove = amount.0;
        assert!(
            valued_actual_shares >= to_remove,
            "Not enough share value to remove the requested amount from the NSLP"
        );
        // Calculate the number of "nslp" shares that the account will burn for removing the given amount of near liquidity from the lp
        let mut num_shares_to_burn = self.nslp_shares_from_amount(to_remove, &nslp_account);
        assert!(num_shares_to_burn > 0);

        //cannot leave less than 1 NEAR
        if valued_actual_shares - to_remove < ONE_NEAR {
            //if less than 1 near left, remove all
            to_remove = valued_actual_shares;
            num_shares_to_burn = acc.nslp_shares;
        }

        //compute proportionals SKASH/UNSTAKED/NEAR
        //1st: SKASH
        let stake_shares_to_remove = proportional(
            nslp_account.stake_shares,
            num_shares_to_burn,
            nslp_account.nslp_shares,
        );
        let skash_to_remove_from_pool = self.amount_from_stake_shares(stake_shares_to_remove);
        //2nd: unstaked in the pool, proportional to shares beign burned
        let unstaked_to_remove = proportional(
            nslp_account.unstaked,
            num_shares_to_burn,
            nslp_account.nslp_shares,
        );
        //3rd: NEAR, by difference
        assert!(
            to_remove >= skash_to_remove_from_pool + unstaked_to_remove,
            "inconsistency NTR<STR+UTR"
        );
        let near_to_remove = to_remove - skash_to_remove_from_pool - unstaked_to_remove;

        //update user account
        //remove first from SKASH in the pool, proportional to shares beign burned
        acc.available += near_to_remove;
        acc.add_stake_shares(stake_shares_to_remove, skash_to_remove_from_pool); //add skash to user acc
        acc.unstaked += unstaked_to_remove;
        acc.nslp_shares -= num_shares_to_burn; //shares this user burns
        //update NSLP account
        nslp_account.available -= near_to_remove;
        nslp_account.remove_stake_shares(stake_shares_to_remove,skash_to_remove_from_pool); //remove skash from the pool
        nslp_account.unstaked -= unstaked_to_remove;
        nslp_account.nslp_shares -= num_shares_to_burn; //burn from total nslp shares

        //--SAVE ACCOUNTS
        self.internal_save_account(&account_id, &acc);
        self.internal_save_account(&NSLP_INTERNAL_ACCOUNT.into(), &nslp_account);
    }


    //------------------
    // HARVEST G-SKASH
    //------------------

    ///g-skash for stakers are realized during stake(), unstake() or by calling harvest_g_skash_from_staking()
    //realize pending g-skash rewards from staking
    pub fn harvest_g_skash_from_staking(&mut self){

        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);

        //realize and mint g-skash
        acc.stake_realize_g_skash(self);

        //--SAVE ACCOUNT
        self.internal_save_account(&account_id, &acc);
    }

    ///g-skash for LP providers are realized during add_liquidit(), remove_liquidity() or by calling harvest_g_skash_from_lp()
    ///realize pending g-skash rewards from LP
    pub fn harvest_g_skash_from_lp(&mut self){

        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);

        //get NSLP account
        let nslp_account = self.internal_get_nslp_account();
        
        //realize and mint g-skash
        acc.nslp_realize_g_skash(&nslp_account, self);
        
        //--SAVE ACCOUNT
        self.internal_save_account(&account_id, &acc);
    }


    //---------------------------------
    // staking-pools-list (SPL) management
    //---------------------------------

    /// get the current list of pools
    pub fn get_staking_pool_list(&self) -> Vec<StakingPoolJSONInfo> {
        let mut result = Vec::with_capacity(self.staking_pools.len());
        for elem in self.staking_pools.iter(){
            result.push(StakingPoolJSONInfo{
                account_id: elem.account_id.clone(),
                weight_basis_points: elem.weight_basis_points,
                staked: elem.staked.into(),
                unstaked: elem.unstaked.into(),
                last_asked_rewards_epoch_height: elem.last_asked_rewards_epoch_height.into(),
                unstaked_requested_epoch_height: elem.unstk_req_epoch_height.into(),
            })
        }
        return result;
    }

    ///remove staking pool from list *if it's empty*
    pub fn remove_staking_pool(&mut self, inx:u16 ){

        self.assert_owner_calling();

        let sp = &self.staking_pools[inx as usize];
        if !sp.is_empty() {
            panic!(b"sp is not empty")
        }
        self.staking_pools.remove(inx as usize);
    }

    ///update existing weight_basis_points
    pub fn set_staking_pool_weight(&mut self, inx:u16, weight_basis_points:u16 ){

        self.assert_owner_calling();

        let sp = &mut self.staking_pools[inx as usize];
        if sp.busy_lock {
            panic!(b"sp is busy")
        }
        sp.weight_basis_points = weight_basis_points;

        self.check_staking_pool_list_consistency();

    }
    
    fn check_staking_pool_list_consistency(&self) {
        let mut total_weight: u16 = 0;
        for sp in self.staking_pools.iter() {
            total_weight+=sp.weight_basis_points;
        }
        assert!(total_weight<=10000,"sum(staking_pools.weight) can not be GT 100%");
    }

    ///add a new staking pool or update existing weight_basis_points
    pub fn set_staking_pool(&mut self, account_id:AccountId, weight_basis_points:u16 ){

        self.assert_owner_calling();

        //search the pools
        for sp in self.staking_pools.iter_mut() {
            if sp.account_id==account_id {
                //found
                if sp.busy_lock {
                    panic!(b"sp is busy")
                }
                (*sp).weight_basis_points = weight_basis_points;
                return;
            }
        }
        //not found, it's a new pool
        self.staking_pools.push(  StakingPoolInfo::new(account_id, weight_basis_points) );

        self.check_staking_pool_list_consistency();
    }

    //--------------------------------------------------
    /// computes unstaking dealy on current situation
    pub fn current_unstaking_delay(&self) -> u16 {
        for inx in 0..self.staking_pools.len()-1 {
            //if there's at leas one pool with no unstaking in process, return standard delay
            if self.staking_pools[inx].staked>0 && self.staking_pools[inx].unstaked==0 { return NUM_EPOCHS_TO_UNLOCK as u16}
        }
        //all pools are in unstaking-delay, it will take double
        return 2* NUM_EPOCHS_TO_UNLOCK as u16; 
    }

    //-----------------------------
    // DISTRIBUTE
    //-----------------------------

    /// operator method
    /// distribute_staking(). Do staking in batches of at most 100Kn
    /// returns "true" if some work was scheduled
    pub fn distribute_staking(&mut self) -> bool {

        self.assert_owner_calling();

        //let epoch_height = env::epoch_height();
        // if self.last_epoch_height == epoch_height {
        //     return false;
        // }
        // self.last_epoch_height = epoch_height;

        //-------------------------------------
        //check if we need to actually stake
        //-------------------------------------
        let mut amount_to_stake = 0;
        if self.total_for_staking > 0 && self.total_for_staking > self.total_actually_staked {
            //more ordered for staking than actually staked
            amount_to_stake = self.total_for_staking - self.total_actually_staked;
        }

        //-------------------------------------
        //check if we need to actually un-stake
        //-------------------------------------
        let mut amount_to_unstake = 0;
        if self.total_for_unstaking > 0 && self.total_for_unstaking > self.total_actually_unstaked {
            //more ordered for unstaking than actually unstaked
            amount_to_unstake = self.total_for_unstaking - self.total_actually_unstaked;
        }

        //-----------------------------------------------
        //internal clearing, no need to talk to the pools
        //-----------------------------------------------
        if amount_to_stake > 0 && amount_to_unstake > 0 {
            if amount_to_stake > amount_to_unstake {
                amount_to_stake -= amount_to_unstake;
                amount_to_unstake = 0;
            } else {
                amount_to_unstake -= amount_to_stake;
                amount_to_stake = 0;
            }
        }

        //-----------------------------------
        //check if we need to actually stake
        //-----------------------------------
        if amount_to_stake > 0 {
            //more ordered for staking than actually staked
            // do it in batches of 100/150k
            if amount_to_stake > MAX_NEARS_SINGLE_MOVEMENT {
                //split movements
                amount_to_stake = NEARS_PER_BATCH;
            }
            let sp_inx = self.get_staking_pool_requiring_stake();
            if sp_inx != usize::MAX {
                //most unbalanced pool found & available
                //launch async stake or deposit_and_stake on that pool

                let sp = &mut self.staking_pools[sp_inx];
                sp.busy_lock = true;

                if sp.unstaked > 0 {
                    //pool has unstaked amount
                    if sp.unstaked < amount_to_stake {
                        amount_to_stake = sp.unstaked;
                    }
                    //launch async stake to re-stake on the pool
                    assert!(sp.unstaked >= amount_to_stake);
                    self.total_actually_staked += amount_to_stake; //preventively consider the amount staked (undoes if failed)
                    ext_staking_pool::stake(
                        amount_to_stake.into(),
                        &sp.account_id,
                        NO_DEPOSIT,
                        gas::staking_pool::STAKE,
                    )
                    .then(ext_self_owner::on_staking_pool_stake_maybe_deposit(
                        sp_inx,
                        amount_to_stake,
                        false,
                        &env::current_account_id(),
                        NO_DEPOSIT,
                        gas::owner_callbacks::ON_STAKING_POOL_DEPOSIT_AND_STAKE,
                    ));

                    return true; //some work scheduled
                }

                //here the sp has no unstaked balance, we must deposit_and_stake on the sp
                //launch async deposit_and_stake on the pool
                assert!(
                    env::account_balance() - MIN_BALANCE_FOR_STORAGE >= amount_to_stake,
                    "env::account_balance()-MIN_BALANCE_FOR_STORAGE < amount_to_deposit_and_stake"
                );
                assert!(
                    self.total_available >= amount_to_stake,
                    "self.available {} .LT. amount_to_deposit_and_stake {}", self.total_available, amount_to_stake
                );
                self.total_available -= amount_to_stake; //preventively consider the amount sent (undoes if async fails)
                self.total_actually_staked += amount_to_stake; //preventively consider the amount staked (undoes if async fails)

                ext_staking_pool::deposit_and_stake(
                    &sp.account_id,
                    amount_to_stake.into(), //attached amount
                    gas::staking_pool::DEPOSIT_AND_STAKE,
                )
                .then(ext_self_owner::on_staking_pool_stake_maybe_deposit(
                    sp_inx,
                    amount_to_stake,
                    true,
                    &env::current_account_id(),
                    NO_DEPOSIT,
                    gas::owner_callbacks::ON_STAKING_POOL_DEPOSIT_AND_STAKE,
                ));

                return true; //some work scheduled
            }
        }

        //-------------------------------------
        //check if we need to actually UN-stake
        //-------------------------------------
        if amount_to_unstake > 0 {
            //more ordered for unstaking than actually unstaked
            //do it in batches of 100/150k
            if amount_to_unstake > MAX_NEARS_SINGLE_MOVEMENT {
                //split movements
                amount_to_unstake = NEARS_PER_BATCH;
            }
            let sp_inx = self.get_staking_pool_requiring_unstake();
            if sp_inx != usize::MAX {
                //most unbalanced pool found & available
                //launch async to unstake

                let sp = &mut self.staking_pools[sp_inx];
                sp.busy_lock = true;

                //max to unstake is amount staked
                if sp.staked < amount_to_unstake {
                    amount_to_unstake = sp.staked;
                }
                //launch async to un-stake from the pool
                assert!(sp.staked >= amount_to_unstake);
                self.total_actually_staked -= amount_to_unstake; //preventively consider the amount un-staked (undoes if promise fails)
                self.total_actually_unstaked += amount_to_unstake; //preventively consider the amount un-staked (undoes if promise fails)
                ext_staking_pool::unstake(
                    amount_to_unstake.into(),
                    &sp.account_id,
                    NO_DEPOSIT,
                    gas::staking_pool::UNSTAKE,
                )
                .then(ext_self_owner::on_staking_pool_unstake(
                    sp_inx,
                    amount_to_unstake,
                    &env::current_account_id(),
                    NO_DEPOSIT,
                    gas::owner_callbacks::ON_STAKING_POOL_UNSTAKE,
                ));

                return true; //some work scheduled
            }
        }

        //try to complete pending unstakes (recover funds if waiting period has elapsed)
        return self.internal_async_withdraw_from_a_pool();
    }

    //prev fn continues here
    /// Called after amount is staked from the sp's unstaked balance (all into  the staking pool contract).
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_stake_maybe_deposit(
        &mut self,
        sp_inx: usize,
        amount: u128,
        included_deposit: bool,
    ) -> bool {

        assert_callback_calling();

        let sp = &mut self.staking_pools[sp_inx];
        sp.busy_lock = false;
        let stake_succeeded = is_promise_success();

        let result: &str;
        if stake_succeeded {
            result = "succeeded";
            if !included_deposit {
                //not deposited first, so staked funds came from unstaked funds already in the sp
                sp.unstaked -= amount;
            }
            //move into staked
            sp.staked += amount;
        } else {
            result = "has failed";
            if included_deposit {
                self.total_available += amount; //undo preventive action considering the amount sent
            }
            self.total_actually_staked -= amount; //undo preventive action considering the amount staked
        }
        env::log(format!("Staking of {} at @{} {}", amount, sp.account_id, result).as_bytes());
        return stake_succeeded;
    }


    /// operator method
    /// distribute_unstaking(). Do unstaking in batches of at most 100Kn
    /// returns "true" if some work was scheduled
    pub fn distribute_unstaking(&mut self) -> bool {

        self.assert_owner_calling();

        //let epoch_height = env::epoch_height();
        // if self.last_epoch_height == epoch_height {
        //     return false;
        // }
        // self.last_epoch_height = epoch_height;

        //-------------------------------------
        //check if we need to actually un-stake
        //-------------------------------------
        let mut amount_to_unstake = 0;
        if self.total_for_unstaking > self.total_actually_unstaked {
            //more ordered for unstaking than actually unstaked
            amount_to_unstake = self.total_for_unstaking - self.total_actually_unstaked;
        }
        else {
            return false;
        }

        //-------------------------------------
        //check if we need to stake also
        //-------------------------------------
        let mut amount_to_stake = 0;
        if self.total_for_staking > self.total_actually_staked {
            //more ordered for staking than actually staked
            amount_to_stake = self.total_for_staking - self.total_actually_staked;
        }

        //-----------------------------------------------
        //internal clearing, no need to talk to the pools
        //-----------------------------------------------
        if amount_to_stake > amount_to_unstake {
            amount_to_stake -= amount_to_unstake;
            amount_to_unstake = 0;
            return false;
        }

        //-----------------------------------
        //check if we need to actually stake
        //-----------------------------------
        if amount_to_stake > 0 {
            //more ordered for staking than actually staked
            // do it in batches of 100/150k
            if amount_to_stake > MAX_NEARS_SINGLE_MOVEMENT {
                //split movements
                amount_to_stake = NEARS_PER_BATCH;
            }
            let sp_inx = self.get_staking_pool_requiring_stake();
            if sp_inx != usize::MAX {
                //most unbalanced pool found & available
                //launch async stake or deposit_and_stake on that pool

                let sp = &mut self.staking_pools[sp_inx];
                sp.busy_lock = true;

                if sp.unstaked > 0 {
                    //pool has unstaked amount
                    if sp.unstaked < amount_to_stake {
                        amount_to_stake = sp.unstaked;
                    }
                    //launch async stake to re-stake on the pool
                    assert!(sp.unstaked >= amount_to_stake);
                    self.total_actually_staked += amount_to_stake; //preventively consider the amount staked (undoes if failed)
                    ext_staking_pool::stake(
                        amount_to_stake.into(),
                        &sp.account_id,
                        NO_DEPOSIT,
                        gas::staking_pool::STAKE,
                    )
                    .then(ext_self_owner::on_staking_pool_stake_maybe_deposit(
                        sp_inx,
                        amount_to_stake,
                        false,
                        &env::current_account_id(),
                        NO_DEPOSIT,
                        gas::owner_callbacks::ON_STAKING_POOL_DEPOSIT_AND_STAKE,
                    ));

                    return true; //some work scheduled
                }

                //here the sp has no unstaked balance, we must deposit_and_stake on the sp
                //launch async deposit_and_stake on the pool
                assert!(
                    env::account_balance() - MIN_BALANCE_FOR_STORAGE >= amount_to_stake,
                    "env::account_balance()-MIN_BALANCE_FOR_STORAGE < amount_to_deposit_and_stake"
                );
                assert!(
                    self.total_available >= amount_to_stake,
                    "self.available {} .LT. amount_to_deposit_and_stake {}", self.total_available, amount_to_stake
                );
                self.total_available -= amount_to_stake; //preventively consider the amount sent (undoes if async fails)
                self.total_actually_staked += amount_to_stake; //preventively consider the amount staked (undoes if async fails)

                ext_staking_pool::deposit_and_stake(
                    &sp.account_id,
                    amount_to_stake.into(), //attached amount
                    gas::staking_pool::DEPOSIT_AND_STAKE,
                )
                .then(ext_self_owner::on_staking_pool_stake_maybe_deposit(
                    sp_inx,
                    amount_to_stake,
                    true,
                    &env::current_account_id(),
                    NO_DEPOSIT,
                    gas::owner_callbacks::ON_STAKING_POOL_DEPOSIT_AND_STAKE,
                ));

                return true; //some work scheduled
            }
        }

        //-------------------------------------
        //check if we need to actually UN-stake
        //-------------------------------------
        if amount_to_unstake > 0 {
            //more ordered for unstaking than actually unstaked
            //do it in batches of 100/150k
            if amount_to_unstake > MAX_NEARS_SINGLE_MOVEMENT {
                //split movements
                amount_to_unstake = NEARS_PER_BATCH;
            }
            let sp_inx = self.get_staking_pool_requiring_unstake();
            if sp_inx != usize::MAX {
                //most unbalanced pool found & available
                //launch async to unstake

                let sp = &mut self.staking_pools[sp_inx];
                sp.busy_lock = true;

                //max to unstake is amount staked
                if sp.staked < amount_to_unstake {
                    amount_to_unstake = sp.staked;
                }
                //launch async to un-stake from the pool
                assert!(sp.staked >= amount_to_unstake);
                self.total_actually_staked -= amount_to_unstake; //preventively consider the amount un-staked (undoes if promise fails)
                self.total_actually_unstaked += amount_to_unstake; //preventively consider the amount un-staked (undoes if promise fails)
                ext_staking_pool::unstake(
                    amount_to_unstake.into(),
                    &sp.account_id,
                    NO_DEPOSIT,
                    gas::staking_pool::UNSTAKE,
                )
                .then(ext_self_owner::on_staking_pool_unstake(
                    sp_inx,
                    amount_to_unstake,
                    &env::current_account_id(),
                    NO_DEPOSIT,
                    gas::owner_callbacks::ON_STAKING_POOL_UNSTAKE,
                ));

                return true; //some work scheduled
            }
        }

        //try to complete pending unstakes (recover funds if waiting period has elapsed)
        return self.internal_async_withdraw_from_a_pool();
    }
    /// Called after the given amount was unstaked at the staking pool contract.
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_unstake(&mut self, sp_inx: usize, amount: u128) -> bool {

        assert_callback_calling();

        let sp = &mut self.staking_pools[sp_inx];
        sp.busy_lock = false;

        let unstake_succeeded = is_promise_success();

        let result: &str;
        if unstake_succeeded {
            result = "succeeded";
            sp.unstaked += amount;
            sp.staked -= amount;
        } else {
            result = "has failed";
            self.total_actually_staked += amount; //undo preventive action considering the amount unstaked
            self.total_actually_unstaked -= amount; //undo preventive action considering the amount unstaked
        }

        env::log(format!("Unstaking of {} at @{} {}", amount, sp.account_id, result).as_bytes());
        return unstake_succeeded;
    }

    //------------------------------------------
    // GETTERS (moved from getters.rs)
    //------------------------------------------
    /// Returns the account ID of the owner.
    pub fn get_operator_account_id(&self) -> AccountId {
        return self.operator_account_id.clone();
    }

    /// The amount of tokens that were deposited to the staking pool.
    /// NOTE: The actual balance can be larger than this known deposit balance due to staking
    /// rewards acquired on the staking pool.
    /// To refresh the amount the owner can call `refresh_staking_pool_balance`.
    pub fn get_known_deposited_balance(&self) -> U128String {
        return self.total_actually_staked.into();
    }

    /// full account info
    /// Returns JSON representation of the account for the given account ID.
    pub fn get_account_info(&self, account_id: AccountId) -> GetAccountInfoResult {
        let acc = self.internal_get_account(&account_id);
        let skash = self.amount_from_stake_shares(acc.stake_shares);
        // trip_rewards = current_skash + trip_accum_unstakes - trip_accum_stakes - trip_start_skash;
        let trip_rewards = (skash + acc.trip_accum_unstakes).saturating_sub(acc.trip_accum_stakes + acc.trip_start_skash);
        //NLSP share value
        let mut nslp_share_value: u128 = 0;
        if acc.nslp_shares != 0 {
            let nslp_account = self.internal_get_nslp_account();
            nslp_share_value = acc.valued_nslp_shares(self, &nslp_account);
        }
        return GetAccountInfoResult {
            account_id,
            available: acc.available.into(),
            skash: skash.into(),
            unstaked: acc.unstaked.into(),
            unstaked_requested_epoch_height: acc.unstaked_requested_epoch_height.into(),
            can_withdraw: (env::epoch_height()
                >= acc.unstaked_requested_epoch_height + NUM_EPOCHS_TO_UNLOCK),
            total: (acc.available + skash + acc.unstaked).into(),
            //trip-meter
            trip_start: acc.trip_start.into(),
            trip_start_skash: acc.trip_start_skash.into(),
            trip_accum_stakes: acc.trip_accum_stakes.into(),
            trip_accum_unstakes: acc.trip_accum_unstakes.into(),
            trip_rewards: trip_rewards.into(),

            nslp_shares: acc.nslp_shares.into(),
            nslp_share_value: nslp_share_value.into(),

            g_skash: acc.total_g_skash(self).into(),
        };
    }


    /// NEP-129 get information about this contract
    /// returns JSON string according to [NEP-129](https://github.com/nearprotocol/NEPs/pull/129)
    pub fn get_contract_info(&self) -> NEP129Response {
        return NEP129Response {
            dataVersion:1,
            name: CONTRACT_NAME.into(),
            version:CONTRACT_VERSION.into(),
            developersAccountId:DEVELOPERS_ACCOUNT_ID.into(),
            source:"https://github.com/Narwallets/diversifying-staking-pool".into(), 
            standards:vec!("NEP-129".into()),  
            webAppUrl:self.web_app_url.clone(),
            auditorAccountId:self.auditor_account_id.clone()
        }
    }

    /// get contract totals 
    /// Returns JSON representation of the contract state
    pub fn get_contract_state(&self) -> GetContractStateResult {

        let lp_account = self.internal_get_nslp_account();

        return GetContractStateResult {
            total_available: self.total_available.into(),
            total_for_staking: self.total_for_staking.into(),
            total_for_unstaking: self.total_for_unstaking.into(),
            total_actually_staked: self.total_actually_staked.into(),
            accumulated_staked_rewards: self.accumulated_staked_rewards.into(),
            total_actually_unstaked: self.total_actually_unstaked.into(),
            total_actually_unstaked_and_retrieved: self.total_actually_unstaked_and_retrieved.into(),
            total_stake_shares: self.total_stake_shares.into(),
            total_g_skash: self.total_g_skash.into(),
            accounts_count: self.accounts.len().into(),
            staking_pools_count: self.staking_pools.len() as u16,
            nslp_liquidity: lp_account.available.into(),
            nslp_current_discount_basis_points: self.internal_get_discount_basis_points(lp_account.available, TEN_NEAR)
        };
    }

    /// Returns JSON representation of contract parameters
    pub fn get_contract_params(&self) -> ContractParamsJSON {
        return ContractParamsJSON {
            staking_paused: self.staking_paused,
            min_account_balance: self.min_account_balance.into(),

            nslp_near_target: self.nslp_near_target.into(),
            nslp_max_discount_basis_points: self.nslp_max_discount_basis_points,
            nslp_min_discount_basis_points: self.nslp_min_discount_basis_points,

            staker_g_skash_mult_pct: self.staker_g_skash_mult_pct,
            skash_sell_g_skash_mult_pct: self.skash_sell_g_skash_mult_pct,
            lp_provider_g_skash_mult_pct: self.lp_provider_g_skash_mult_pct,
                    
            operator_rewards_fee_basis_points: self.operator_rewards_fee_basis_points,
            operator_swap_cut_basis_points: self.operator_swap_cut_basis_points,
            treasury_swap_cut_basis_points: self.treasury_swap_cut_basis_points,
            };
    }

    /// Returns JSON representation of contract parameters
    pub fn set_contract_params(&mut self, params:ContractParamsJSON) {

        self.assert_owner_calling();

        self.min_account_balance = params.min_account_balance.0;

        self.nslp_near_target = params.nslp_near_target.0;
        self.nslp_max_discount_basis_points = params.nslp_max_discount_basis_points;
        self.nslp_min_discount_basis_points = params.nslp_min_discount_basis_points;

        self.staker_g_skash_mult_pct = params.staker_g_skash_mult_pct;
        self.skash_sell_g_skash_mult_pct = params.skash_sell_g_skash_mult_pct;
        self.lp_provider_g_skash_mult_pct = params.lp_provider_g_skash_mult_pct;
                    
        self.operator_rewards_fee_basis_points = params.operator_rewards_fee_basis_points;
        self.operator_swap_cut_basis_points = params.operator_swap_cut_basis_points;
        self.treasury_swap_cut_basis_points = params.treasury_swap_cut_basis_points;

    }
    
    /// get sp (staking-pool) info
    /// Returns JSON representation of sp recorded state
    pub fn get_sp_info(&self, sp_inx_i32: i32) -> StakingPoolJSONInfo {

        assert!(sp_inx_i32 > 0);

        let sp_inx = sp_inx_i32 as usize;
        assert!(sp_inx < self.staking_pools.len());

        let sp = &self.staking_pools[sp_inx];

        return StakingPoolJSONInfo {
            account_id: sp.account_id.clone(),
            weight_basis_points: sp.weight_basis_points,
            staked: sp.staked.into(),
            unstaked: sp.unstaked.into(),
            unstaked_requested_epoch_height: sp.unstk_req_epoch_height.into(),
            last_asked_rewards_epoch_height: sp.last_asked_rewards_epoch_height.into(),
        };
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use near_sdk::{testing_env, MockedBlockchain, PromiseResult, VMContext};

    use test_utils::*;

    use super::*;

    mod test_utils;

    pub type AccountId = String;

    const SALT: [u8; 3] = [1, 2, 3];

    fn basic_context() -> VMContext {
        get_context(
            system_account(),
            to_yocto(TEST_INITIAL_BALANCE),
            0,
            to_ts(GENESIS_TIME_IN_DAYS),
            false,
        )
    }

    fn new_contract() -> DiversifiedPool {
        DiversifiedPool::new(account_owner())
    }

    fn contract_only_setup() -> (VMContext, DiversifiedPool) {
        let context = basic_context();
        testing_env!(context.clone());
        let contract = new_contract();
        return (context, contract);
    }

    // #[test]
    // fn test_gfme_only_basic() {
    //     let (mut context, contract) = contract_only_setup();
    //     // Checking initial values at genesis time
    //     context.is_view = true;
    //     testing_env!(context.clone());

    //     assert_eq!(contract.get_owners_balance().0, 0);

    //     // Checking values in 1 day after genesis time
    //     context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 1);

    //     assert_eq!(contract.get_owners_balance().0, 0);

    //     // Checking values next day after gfme timestamp
    //     context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
    //     testing_env!(context.clone());

    //     assert_almost_eq(contract.get_owners_balance().0, to_yocto(TEST_INITIAL_BALANCE));
    // }
    #[test]
    fn test_internal_get_near_amount_sell_skash() {
        let (mut context, mut contract) = contract_only_setup();
        let lp_balance_y: u128 = to_yocto(500_000);
        let sell_skash_y: u128 = to_yocto(120);
        let discount_bp: u16 = contract.get_discount_basis_points(lp_balance_y, sell_skash_y);
        let near_amount_y =
            contract.internal_get_near_amount_sell_skash(lp_balance_y, sell_skash_y);
        assert!(near_amount_y <= sell_skash_y);
        let discountedy = sell_skash_y - near_amount_y;
        let discounted_displayN = ytof(discountedy);
        let sell_skash_displayN = ytof(sell_skash_y);
        assert!(discountedy == apply_pct(discount_bp, sell_skash_y));
        assert!(near_amount_y == sell_skash_y - discountedy);
    }

    /*
    #[test]
    fn test_add_full_access_key() {
        let (mut context, mut contract) = contract_only_setup();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR);
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(1).try_into().unwrap();
        testing_env!(context.clone());

        contract.add_full_access_key(public_key(4));
    }

    #[test]
    #[should_panic(expected = "Can only be called by the owner")]
    fn test_call_by_non_owner() {
        let (mut context, mut contract) = contract_only_setup();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR);
        context.predecessor_account_id = non_owner();
        context.signer_account_id = non_owner();
        testing_env!(context.clone());

        contract.select_staking_pool(AccountId::from("staking_pool"));
    }


    #[test]
    fn test_gfme_only_transfer_call_by_owner() {
        let (mut context, mut contract) = contract_only_setup();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
        context.is_view = true;
        testing_env!(context.clone());
        assert_almost_eq(contract.get_owners_balance().0, to_yocto(TEST_INITIAL_BALANCE));

        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(1).try_into().unwrap();
        context.is_view = false;
        testing_env!(context.clone());

        assert_eq!(env::account_balance(), to_yocto(TEST_INITIAL_BALANCE));
        contract.transfer(to_yocto(100).into(), non_owner());
        assert_almost_eq(env::account_balance(), to_yocto(TEST_INITIAL_BALANCE - 100));
    }

    #[test]
    #[should_panic(expected = "Staking pool is not selected")]
    fn test_staking_pool_is_not_selected() {
        let (mut context, mut contract) = contract_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        let amount = to_yocto(TEST_INITIAL_BALANCE - 100);
        testing_env!(context.clone());
        contract.deposit_to_staking_pool(amount.into());
    }

    #[test]
    fn test_staking_pool_success() {
        let (mut context, mut contract) = contract_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        // Selecting staking pool
        let staking_pool = "staking_pool".to_string();
        testing_env!(context.clone());
        contract.select_staking_pool(staking_pool.clone());

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_staking_pool_account_id(), Some(staking_pool));
        assert_eq!(contract.get_known_deposited_balance().0, 0);
        context.is_view = false;

        // Deposit to the staking_pool
        let amount = to_yocto(TEST_INITIAL_BALANCE - 100);
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.deposit_to_staking_pool(amount.into());
        context.account_balance = env::account_balance();
        assert_eq!(context.account_balance, to_yocto(TEST_INITIAL_BALANCE) - amount);

        context.predecessor_account_id = gfme_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_deposit(amount.into());
        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_known_deposited_balance().0, amount);
        context.is_view = false;

        // Staking on the staking pool
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.stake(amount.into());

        context.predecessor_account_id = gfme_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_stake(amount.into());

        // Assuming there are 20 NEAR tokens in rewards. Unstaking.
        let unstake_amount = amount + to_yocto(20);
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.unstake(unstake_amount.into());

        context.predecessor_account_id = gfme_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_unstake(unstake_amount.into());

        // Withdrawing
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.withdraw_from_staking_pool(unstake_amount.into());
        context.account_balance += unstake_amount;

        context.predecessor_account_id = gfme_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_withdraw(unstake_amount.into());
        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_known_deposited_balance().0, 0);
        context.is_view = false;

        // Unselecting staking pool
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.unselect_staking_pool();
        assert_eq!(contract.get_staking_pool_account_id(), None);
    }

    #[test]
    fn test_staking_pool_refresh_balance() {
        let (mut context, mut contract) = contract_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        // Selecting staking pool
        let staking_pool = "staking_pool".to_string();
        testing_env!(context.clone());
        contract.select_staking_pool(staking_pool.clone());

        // Deposit to the staking_pool
        let amount = to_yocto(TEST_INITIAL_BALANCE - 100);
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.deposit_to_staking_pool(amount.into());
        context.account_balance = env::account_balance();
        assert_eq!(context.account_balance, to_yocto(TEST_INITIAL_BALANCE) - amount);

        context.predecessor_account_id = gfme_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_deposit(amount.into());

        // Staking on the staking pool
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.stake(amount.into());

        context.predecessor_account_id = gfme_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_stake(amount.into());

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, 0);
        assert_eq!(contract.get_liquid_owners_balance().0, 0);
        assert_eq!(contract.get_known_deposited_balance().0, amount);
        context.is_view = false;

        // Assuming there are 20 NEAR tokens in rewards. Refreshing balance.
        let total_balance = amount + to_yocto(20);
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.refresh_staking_pool_balance();

        // In unit tests, the following call ignores the promise value, because it's passed directly.
        context.predecessor_account_id = gfme_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_get_sp_total_balance(sp_account, total_balance.into());

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_known_deposited_balance().0, total_balance);
        assert_eq!(contract.get_owners_balance().0, to_yocto(20));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(20));
        context.is_view = false;

        // Withdrawing these tokens
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        let transfer_amount = to_yocto(15);
        contract.transfer(transfer_amount.into(), non_owner());
        context.account_balance = env::account_balance();

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_known_deposited_balance().0, total_balance);
        assert_eq!(contract.get_owners_balance().0, to_yocto(5));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(5));
        context.is_view = false;
    }

    #[test]
    #[should_panic(expected = "Staking pool is already selected")]
    fn test_staking_pool_selected_again() {
        let (mut context, mut contract) = contract_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        // Selecting staking pool
        let staking_pool = "staking_pool".to_string();
        testing_env!(context.clone());
        contract.select_staking_pool(staking_pool.clone());

        // Selecting another staking pool
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.select_staking_pool("staking_pool_2".to_string());
    }


    #[test]
    #[should_panic(expected = "Staking pool is not selected")]
    fn test_staking_pool_unselecting_non_selected() {
        let (mut context, mut contract) = contract_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        // Unselecting staking pool
        testing_env!(context.clone());
        contract.unselect_staking_pool();
    }


    #[test]
    fn test_staking_pool_owner_balance() {
        let (mut context, mut contract) = contract_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);

        let gfme_amount = to_yocto(TEST_INITIAL_BALANCE);
        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, gfme_amount);
        context.is_view = false;

        // Selecting staking pool
        let staking_pool = "staking_pool".to_string();
        testing_env!(context.clone());
        contract.select_staking_pool(staking_pool.clone());

        // Deposit to the staking_pool
        let mut total_amount = 0;
        let amount = to_yocto(100);
        for _ in 1..=5 {
            total_amount += amount;
            context.predecessor_account_id = account_owner();
            testing_env!(context.clone());
            contract.deposit_to_staking_pool(amount.into());
            context.account_balance = env::account_balance();
            assert_eq!(context.account_balance, gfme_amount - total_amount);

            context.predecessor_account_id = gfme_account();
            testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
            contract.on_staking_pool_deposit(amount.into());
            context.is_view = true;
            testing_env!(context.clone());
            assert_eq!(contract.get_known_deposited_balance().0, total_amount);
            assert_eq!(contract.get_owners_balance().0, gfme_amount);
            assert_eq!(
                contract.get_liquid_owners_balance().0,
                gfme_amount - total_amount - MIN_BALANCE_FOR_STORAGE
            );
            context.is_view = false;
        }

        // Withdrawing from the staking_pool. Plus one extra time as a reward
        let mut total_withdrawn_amount = 0;
        for _ in 1..=6 {
            total_withdrawn_amount += amount;
            context.predecessor_account_id = account_owner();
            testing_env!(context.clone());
            contract.withdraw_from_staking_pool(amount.into());
            context.account_balance += amount;
            assert_eq!(
                context.account_balance,
                gfme_amount - total_amount + total_withdrawn_amount
            );

            context.predecessor_account_id = gfme_account();
            testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
            contract.on_staking_pool_withdraw(amount.into());
            context.is_view = true;
            testing_env!(context.clone());
            assert_eq!(
                contract.get_known_deposited_balance().0,
                total_amount.saturating_sub(total_withdrawn_amount)
            );
            assert_eq!(
                contract.get_owners_balance().0,
                gfme_amount + total_withdrawn_amount.saturating_sub(total_amount)
            );
            assert_eq!(
                contract.get_liquid_owners_balance().0,
                gfme_amount - total_amount + total_withdrawn_amount - MIN_BALANCE_FOR_STORAGE
            );
            context.is_view = false;
        }
    }
    */
}
