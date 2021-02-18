//! A smart contract that allows diversified staking, stNEAR and META farming
//! this contract include parts of core-contracts/lockup-contract & core-contracts/staking-pool

/********************************/
/* CONTRACT Self Identification */
/********************************/
// [NEP-129](https://github.com/nearprotocol/NEPs/pull/129)
// see also pub fn get_contract_info
const CONTRACT_NAME: &str = "Meta Staking Pool";
const CONTRACT_VERSION: &str = "0.1.0";
const DEFAULT_WEB_APP_URL: &str = "https://www.narwallets.com/dapp/mainnet/meta/";
const DEFAULT_AUDITOR_ACCOUNT_ID: &str = "auditors.near";

use near_sdk::{env, ext_contract, near_bindgen, AccountId};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::Base58PublicKey;
use near_sdk::collections::{UnorderedMap,LookupMap};

pub use crate::account::*;
pub use crate::internal::*;
pub use crate::owner::*;
pub use crate::getters::*;
pub use crate::types::*;
pub use crate::utils::*;

pub mod gas;
pub mod types;
pub mod utils;
pub mod getters;

pub mod account;
pub mod internal;
pub mod distribute;
pub mod owner;
pub mod multi_fun_token;

pub mod reward_meter;
pub use reward_meter::*;

pub mod validator_loans;
pub use validator_loans::*;

#[cfg(target = "wasm32")]
#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc = near_sdk::wee_alloc::WeeAlloc::INIT;

pub const NSLP_INTERNAL_ACCOUNT: &str = "..NSLP..";

macro_rules! debug {
    ($($arg:tt)*) => ({
        env::log(format!($($arg)*).as_bytes());
    });
}


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
pub trait ExtMetaStakingPoolOwnerCallbacks {
    fn on_staking_pool_deposit(&mut self, amount: U128String) -> bool;

    fn on_staking_pool_withdraw(&mut self, inx: u16) -> bool;

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
pub struct MetaPool {
    /// Owner's account ID (it will be a DAO on phase II)
    pub owner_account_id: String,

    /// if you're holding stnear there's a min balance you must mantain to backup storage usage
    /// can be adjusted down by keeping the required NEAR in the developers or operator account
    pub min_account_balance: u128,

    // Configurable info for [NEP-129](https://github.com/nearprotocol/NEPs/pull/129)
    pub web_app_url: Option<String>, 
    pub auditor_account_id: Option<String>,

    /// This amount increments with deposits and decrements when users stake
    /// increments with finish_unstake and decrements with user withdrawals from the contract
    /// since staking/unstaking is delayed it only eventually matches env::balance()
    pub total_available: u128,

    /// The total amount of tokens selected for staking by the users
    /// not necessarily what's actually staked since staking can is done in batches
    /// Share price is computed using this number. share_price = total_for_staking/total_shares
    pub total_for_staking: u128,

    /// The total amount of tokens actually staked (the tokens are in the staking pools)
    // During distribute_staking(), If !staking_paused && total_for_staking<total_actually_staked, then the difference gets staked in the pools
    // During distribute_unstaking(), If total_actually_staked>total_for_staking, then the difference gets unstaked from the pools
    pub total_actually_staked: u128,

    /// how many "shares" were minted. Everytime someone "stakes" he "buys pool shares" with the staked amount
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
    /// It represents funds retrieved from the pools, but waiting for the users to execute finish_unstake()
    /// During distribute_unstake(), If sp.unstaked>0 && sp.epoch_for_withdraw == env::epoch_height then all unstaked funds are retrieved from the sp
    /// When the funds are actually requested by the users, total_actually_unstaked is decremented
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

    //validator's loan requests
    pub loan_requests: LookupMap<String, VLoanRequest>,

    //The next 3 values define the Liq.Provider fee curve
    // NEAR/stNEAR Liquidity pool fee curve params
    // We assume this pool is always UNBALANCED, there should be more stNEAR than NEAR 99% of the time
    ///NEAR/stNEAR 1% fee Liquidity target. If the Liquidity reach this amount, the fee is 1%
    pub nslp_near_one_percent_target: u128, // 150_000*NEAR initially
    ///NEAR/stNEAR Liquidity pool max fee
    pub nslp_max_discount_basis_points: u16, //5% initially
    ///NEAR/stNEAR Liquidity pool min fee
    pub nslp_min_discount_basis_points: u16, //0.5% initially

    //The next 3 values define meta rewards multiplers %. (100 => 1x, 200 => 2x, ...)
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
    /// owner's cut on SHKASH Sell fee (3% default)
    pub operator_swap_cut_basis_points: u16,
    /// Treasury account ID (it will be controlled by a DAO on phase II)
    pub treasury_account_id: String,
    /// treasury cut on SHKASH Sell cut (25% default)
    pub treasury_swap_cut_basis_points: u16,
}

impl Default for MetaPool {
    fn default() -> Self {
        env::panic(b"The contract is not initialized.");
    }
}

#[near_bindgen]
impl MetaPool {
    /* NOTE
    This contract implements several traits

    1. deposit-trait [NEP-xxx]: this contract implements: deposit, get_account_total_balance, get_account_available_balance, withdraw, withdraw_all
       A [NEP-xxx] contract creates an account on deposit and allows you to withdraw later under certain conditions. Deletes the account on withdraw_all

    2. staking-pool [NEP-xxx]: this contract must be perceived as a staking-pool for the lockup-contract, wallets, and users.
        This means implmenting: ping, deposit, deposit_and_stake, withdraw_all, withdraw, stake_all, stake, unstake_all, unstake
        and view methods: get_account_unstaked_balance, get_account_staked_balance, get_account_total_balance, is_account_unstaked_balance_available,
            get_total_staked_balance, get_owner_id, get_reward_fee_fraction, is_staking_paused, get_staking_key, get_account,
            get_number_of_accounts, get_accounts.

    3. meta-staking: these are the extensions to the standard staking pool (buy/sell stnear, finish_unstake)

    4. multitoken (TODO) [NEP-xxx]: this contract implements: deposit(tok), get_token_balance(tok), withdraw_token(tok), tranfer_token(tok), transfer_token_to_contract(tok)
       A [NEP-xxx] manages multiple tokens

    */

    /// Requires 25 TGas (1 * BASE_GAS)
    ///
    /// Initializes MetaPool contract.
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
            min_account_balance: NEAR,
            web_app_url: Some(String::from(DEFAULT_WEB_APP_URL)),
            auditor_account_id: Some(String::from(DEFAULT_AUDITOR_ACCOUNT_ID)),
            operator_rewards_fee_basis_points: DEFAULT_OPERATOR_REWARDS_FEE_BASIS_POINTS,
            operator_swap_cut_basis_points: DEFAULT_OPERATOR_SWAP_CUT_BASIS_POINTS,
            treasury_swap_cut_basis_points: DEFAULT_TREASURY_SWAP_CUT_BASIS_POINTS,
            staking_paused: false, 
            total_available: 0,
            total_for_staking: 0,
            total_actually_staked: 0, //amount actually sent to the staking_pool and staked
            total_unstaked_and_waiting: 0, // tracks unstaked amount from the staking_pool (toekns are in the pool)
            total_actually_unstaked_and_retrieved: 0, // tracks unstaked AND retrieved amount (tokens are here)
            accumulated_staked_rewards: 0,
            total_stake_shares: 0,
            total_meta: 0,
            accounts: UnorderedMap::new("A".into()),
            loan_requests: LookupMap::new("L".into()),
            nslp_near_one_percent_target: 150_000*NEAR,
            nslp_max_discount_basis_points: 500, //5%
            nslp_min_discount_basis_points: 50,   //0.5%
            ///for each stNEAR paid as discount, reward stNEAR sellers with g-stNEAR. default:1x. reward META = discounted * mult_pct / 100
            stnear_sell_meta_mult_pct: 100, //1x
            ///for each stNEAR paid staking reward, reward stNEAR holders with g-stNEAR. default:5x. reward META = rewards * mult_pct / 100
            staker_meta_mult_pct: 500, //5x
            ///for each stNEAR paid as discount, reward LPs with g-stNEAR. default:20x. reward META = fee * mult_pct / 100
            lp_provider_meta_mult_pct: 2000, //20x
            staking_pools: Vec::new(),

        };
    }

    //pub fn set_min_balance(&mut self)

    //------------------------------------
    // deposit trait & staking-pool trait
    //------------------------------------

    /// staking-pool's ping is moot here
    pub fn ping(&mut self) {
        
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
    // DIVERISIFYING-STAKING-POOL trait
    //----------------------------------
    //----------------------------------

    /// Returns the list of accounts with full data (div-pool trait)
    pub fn get_accounts_info(&self, from_index: u64, limit: u64) -> Vec<GetAccountInfoResult> {
        let keys = self.accounts.keys_as_vector();
        return (from_index..std::cmp::min(from_index + limit, keys.len()))
            .map(|index| self.get_account_info(keys.get(index).unwrap()))
            .collect();
    }


    /// user method
    /// completes unstake action by moving from retreieved_from_the_pools to available
    pub fn finish_unstaking(&mut self) {

        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);

        account.try_finish_unstaking(self);

        self.internal_update_account(&account_id, &account);

        env::log(
            format!(
                "@{} finishing unstaking. New available balance is {}",
                account_id, account.available
            )
            .as_bytes(),
        );
    }

    /// buy_stnear_stake. Identical to stake, migth change in the future
    pub fn buy_stnear_stake(&mut self, amount: U128String) {
        self.internal_stake(amount.0);
    }

    //---------------------------
    // NSLP Methods
    //---------------------------

    /// user method - NEAR/stNEAR SWAP functions
    /// return how much NEAR you can get by selling x stNEAR
    pub fn get_near_amount_sell_stnear(&self, stnear_to_sell: U128String) -> U128String {
        let lp_account = self.internal_get_nslp_account();
        return self.internal_get_near_amount_sell_stnear(lp_account.available, stnear_to_sell.0).into();
    }

    /// NEAR/stNEAR Liquidity Pool
    /// computes the discount_basis_points for NEAR/stNEAR Swap based on NSLP Balance
    /// If you want to sell x stNEAR
    pub fn nslp_get_discount_basis_points(&self, stnear_to_sell: U128String) -> u16 {
        let lp_account = self.internal_get_nslp_account();
        return self.internal_get_discount_basis_points(lp_account.available, stnear_to_sell.0);
    }

    /// user method
    /// Sells-stnear at discount in the NLSP
    /// returns near received
    pub fn sell_stnear(
        &mut self,
        stnear_to_sell: U128String,
        min_expected_near: U128String,
    ) -> U128String {
        let account_id = env::predecessor_account_id();
        let mut user_account = self.internal_get_account(&account_id);

        let stnear_owned = self.amount_from_stake_shares(user_account.stake_shares);
        assert!(
            stnear_owned >= stnear_to_sell.0,
            "Not enough stnear in your account"
        );
        //cannot leave less than 1 NEAR
        let to_sell = if stnear_owned - stnear_to_sell.0 < NEAR {
            //if less than 1 near left, sell all
            stnear_owned
        }
        else {
            stnear_to_sell.0
        };

        let mut nslp_account = self.internal_get_nslp_account();
        let near_to_receive =
            self.internal_get_near_amount_sell_stnear(nslp_account.available, to_sell);
        assert!(
            near_to_receive >= min_expected_near.0,
            "Price changed, your min results requirements {} not satisfied {}. Try again", min_expected_near.0, near_to_receive
        );
        assert!(
            nslp_account.available >= near_to_receive,
            "available < near_to_receive"
        );

        let stake_shares_sell = self.stake_shares_from_amount(to_sell);
        assert!(
            user_account.stake_shares >= stake_shares_sell,
            "account.stake_shares < stake_shares_sell"
        );

        //the available for the user comes from the LP
        nslp_account.available -= near_to_receive;
        user_account.available += near_to_receive;

        //the fee is the difference between stnear sold and near received
        assert!(near_to_receive < to_sell);
        let fee_in_stnear = to_sell - near_to_receive;
        // compute how many shares the swap fee represent
        let fee_in_shares = self.stake_shares_from_amount(fee_in_stnear);

        // involved accounts
        let mut treasury_account = self.internal_get_account(&self.treasury_account_id);
        let mut operator_account = self.internal_get_account(&self.operator_account_id);
        let mut developers_account = self.internal_get_account(&DEVELOPERS_ACCOUNT_ID.into());

        // The treasury cut in stnear-shares (25% by default)
        let treasury_stake_shares_cut = apply_pct(self.treasury_swap_cut_basis_points,fee_in_shares);
        let treasury_stnear_cut = apply_pct(self.treasury_swap_cut_basis_points,fee_in_stnear);
        treasury_account.add_stake_shares(treasury_stake_shares_cut,treasury_stnear_cut);
        
        // The cut that the contract owner (operator) takes. (3% of 1% normally)
        let operator_stake_shares_cut = apply_pct( self.operator_swap_cut_basis_points,fee_in_shares);
        let operator_stnear_cut = apply_pct( self.operator_swap_cut_basis_points, fee_in_stnear);
        operator_account.add_stake_shares(operator_stake_shares_cut,operator_stnear_cut);

        // The cut that the developers take. (2% of 1% normally)
        let developers_stake_shares_cut = apply_pct(DEVELOPERS_SWAP_CUT_BASIS_POINTS, fee_in_shares);
        let developers_stnear_cut = apply_pct(DEVELOPERS_SWAP_CUT_BASIS_POINTS, fee_in_stnear);
        developers_account.add_stake_shares(developers_stake_shares_cut,developers_stnear_cut);

        // all the realized meta from non-liq.provider cuts (30%), send to operator & developers
        let stnear_non_lp_cut = treasury_stnear_cut+operator_stnear_cut+developers_stnear_cut;
        let meta_from_operation = apply_multiplier(stnear_non_lp_cut, self.lp_provider_meta_mult_pct);
        self.total_meta += meta_from_operation;
        operator_account.realized_meta += meta_from_operation/2;
        developers_account.realized_meta += meta_from_operation/2;

        debug!("treasury_stake_shares_cut:{} operator_stake_shares_cut:{} developers_stake_shares_cut:{} fee_in_stake_shares:{}",
            treasury_stake_shares_cut,operator_stake_shares_cut,developers_stake_shares_cut,fee_in_shares);

        debug!("treasury_stnear_cut:{} operator_stnear_cut:{} developers_stnear_cut:{} fee_in_stnear:{} stnear_non_lp_cut:{} ",
            treasury_stnear_cut,operator_stnear_cut,developers_stnear_cut,fee_in_stnear,stnear_non_lp_cut);

        assert!(fee_in_shares > treasury_stake_shares_cut + developers_stake_shares_cut + operator_stake_shares_cut);

        // The rest of the stnear sold goes into the LP. Because it is a larger number than NEAR removes, it will increase share value for all LP providers.
        // Adding value to the pool via adding more stnear than the near removed, will be counted as rewards for the nslp_meter, 
        // so meta for LP providers will be created. G-stnear for LP providers are realized during add_liquidit(), remove_liquidity() or by calling harvest_meta_from_lp()
        debug!("nslp_account.add_stake_shares {} {}",
            stake_shares_sell - (treasury_stake_shares_cut + operator_stake_shares_cut + developers_stake_shares_cut),
            to_sell - (treasury_stnear_cut + operator_stnear_cut + developers_stnear_cut));

        // major part of stnear sold goes to the NSLP
        nslp_account.add_stake_shares( 
            stake_shares_sell - (treasury_stake_shares_cut + operator_stake_shares_cut + developers_stake_shares_cut),
            to_sell - (treasury_stnear_cut + operator_stnear_cut + developers_stnear_cut ));

        //complete the transfer, remove stnear from the user (stnear was transferred to the LP & others)
        user_account.sub_stake_shares(stake_shares_sell, to_sell);
        { //give the selling user some meta too
            let meta_to_seller = apply_multiplier(fee_in_stnear, self.stnear_sell_meta_mult_pct);
            self.total_meta += meta_to_seller;
            user_account.realized_meta += meta_to_seller;
        }

        //Save involved accounts
        self.internal_update_account(&self.treasury_account_id.clone(), &treasury_account);
        self.internal_update_account(&self.operator_account_id.clone(), &operator_account);
        self.internal_update_account(&DEVELOPERS_ACCOUNT_ID.into(), &developers_account);
        //Save user and nslp accounts
        self.internal_update_account(&account_id, &user_account);
        self.internal_save_nslp_account(&nslp_account);

        env::log(
            format!(
                "@{} sold {} stNEAR for {} NEAR",
                account_id, to_sell, near_to_receive
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

        //use this LP operation to realize meta pending rewards (same as nslp_harvest_meta)
        acc.nslp_realize_meta(&nslp_account, self);

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
        self.internal_update_account(&account_id, &acc);
        self.internal_save_nslp_account(&nslp_account);
    }

    /// remove liquidity from deposited funds
    pub fn nslp_remove_liquidity(&mut self, amount: U128String) {
        
        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);
        let mut nslp_account = self.internal_get_nslp_account();

        //use this LP operation to realize meta pending rewards (same as nslp_harvest_meta)
        acc.nslp_realize_meta(&nslp_account, self);

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
        if valued_actual_shares - to_remove < NEAR {
            //if less than 1 near left, remove all
            to_remove = valued_actual_shares;
            num_shares_to_burn = acc.nslp_shares;
        }

        //compute proportionals stNEAR/UNSTAKED/NEAR
        //1st: stNEAR
        let stake_shares_to_remove = proportional(
            nslp_account.stake_shares,
            num_shares_to_burn,
            nslp_account.nslp_shares,
        );
        let stnear_to_remove_from_pool = self.amount_from_stake_shares(stake_shares_to_remove);
        //2nd: unstaked in the pool, proportional to shares beign burned
        let unstaked_to_remove = proportional(
            nslp_account.unstaked,
            num_shares_to_burn,
            nslp_account.nslp_shares,
        );
        //3rd: NEAR, by difference
        assert!(
            to_remove >= stnear_to_remove_from_pool + unstaked_to_remove,
            "inconsistency NTR<STR+UTR"
        );
        let near_to_remove = to_remove - stnear_to_remove_from_pool - unstaked_to_remove;

        //update user account
        //remove first from stNEAR in the pool, proportional to shares beign burned
        acc.available += near_to_remove;
        acc.add_stake_shares(stake_shares_to_remove, stnear_to_remove_from_pool); //add stnear to user acc
        acc.unstaked += unstaked_to_remove;
        acc.nslp_shares -= num_shares_to_burn; //shares this user burns
        //update NSLP account
        nslp_account.available -= near_to_remove;
        nslp_account.sub_stake_shares(stake_shares_to_remove,stnear_to_remove_from_pool); //remove stnear from the pool
        nslp_account.unstaked -= unstaked_to_remove;
        nslp_account.nslp_shares -= num_shares_to_burn; //burn from total nslp shares

        //--SAVE ACCOUNTS
        self.internal_update_account(&account_id, &acc);
        self.internal_save_nslp_account(&nslp_account);
    }


    //------------------
    // HARVEST META
    //------------------

    ///meta rewards for stakers are realized during stake(), unstake() or by calling harvest_meta_from_staking()
    //realize pending meta rewards from staking
    pub fn harvest_meta_from_staking(&mut self){

        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);

        //realize and mint meta
        acc.stake_realize_meta(self);

        //--SAVE ACCOUNT
        self.internal_update_account(&account_id, &acc);
    }

    ///meta rewards for LP providers are realized during add_liquidit(), remove_liquidity() or by calling harvest_meta_from_lp()
    ///realize pending meta rewards from LP
    pub fn harvest_meta_from_lp(&mut self){

        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);

        //get NSLP account
        let nslp_account = self.internal_get_nslp_account();
        
        //realize and mint meta
        acc.nslp_realize_meta(&nslp_account, self);
        
        //--SAVE ACCOUNT
        self.internal_update_account(&account_id, &acc);
    }





}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    //use std::convert::TryInto;

/*    use near_sdk::{testing_env, MockedBlockchain,  VMContext}; //PromiseResult,

    use test_utils::*;

    use super::*;

    mod test_utils;

    //pub type AccountId = String;

    //const SALT: [u8; 3] = [1, 2, 3];

    fn basic_context() -> VMContext {
        get_context(
            system_account(),
            to_yocto(TEST_INITIAL_BALANCE),
            0,
            to_ts(GENESIS_TIME_IN_DAYS),
            false,
        )
    }

    fn new_contract() -> MetaPool {
        MetaPool::new(account_owner(), account_owner(), account_owner())
    }

    fn contract_only_setup() -> (VMContext, MetaPool) {
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
    fn test_internal_get_near_amount_sell_stnear() {
        let (_context, contract) = contract_only_setup();
        let lp_balance_y: u128 = to_yocto(500_000);
        let sell_stnear_y: u128 = to_yocto(120);
        let discount_bp: u16 = contract.internal_get_discount_basis_points(lp_balance_y, sell_stnear_y);
        let near_amount_y =
            contract.internal_get_near_amount_sell_stnear(lp_balance_y, sell_stnear_y);
        assert!(near_amount_y <= sell_stnear_y);
        let discountedy = sell_stnear_y - near_amount_y;
        let _discounted_display_n = ytof(discountedy);
        let _sell_stnear_display_n = ytof(sell_stnear_y);
        assert!(discountedy == apply_pct(discount_bp, sell_stnear_y));
        assert!(near_amount_y == sell_stnear_y - discountedy);
    }
*/

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
