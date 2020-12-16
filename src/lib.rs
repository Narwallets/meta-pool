//! A smart contract that allows diversified staking
//! this contract is based on core-contracts/lockup-contract & core-contracts/staking-pool

/*
Notes: 
    In order to keep the skaing balanced over the pools
    large operations (>100kN) wil be splitted
    The cross contract calls are complex and can consume all allowed gas,
    so tha operations will be completed by calling "ping" if total_to_stake < total_staked
*/

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::Base58PublicKey;
use near_sdk::{env, ext_contract, near_bindgen, AccountId, collections::UnorderedMap};

pub use crate::getters::*;
pub use crate::internal::*;
pub use crate::owner::*;
pub use crate::types::*;
pub use crate::utils::*;

pub mod gas;
pub mod types;
pub mod utils;

pub mod getters;
pub mod internal;
pub mod owner;

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc = near_sdk::wee_alloc::WeeAlloc::INIT;


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

    fn on_staking_pool_stake_maybe_deposit(&mut self, sp_inx: usize, amount: u128, include_deposit:bool) -> bool;

    fn on_staking_pool_unstake(&mut self, sp_inx: usize, amount: u128) -> bool;

    //fn on_staking_pool_unstake_all(&mut self) -> bool;

    fn on_get_result_from_transfer_poll(&mut self, #[callback] poll_result: PollResult) -> bool;

    fn on_get_sp_total_balance(&mut self, sp_inx: usize, #[callback] total_balance: U128String);

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
    /// The share_price can be computed as total_staked/total_stake_shares
    /// shares * share_price = SKASHs
    pub stake_shares: u128,

    /// Incremented when the user asks for unstaking. The amount of unstaked near in the pools 
    pub unstaked: u128,

    /// The epoch height when the unstaked was requested
    /// The fund will be locked for NUM_EPOCHS_TO_UNLOCK epochs
    /// unlock epoch = unstaked_requested_epoch_height + NUM_EPOCHS_TO_UNLOCK 
    pub unstaked_requested_epoch_height: EpochHeight,

    //-- STATISTICAL DATA --
    // User's statistical data
    // These fields works as a car's "trip meter". The user can reset them to zero.
    // to compute trip_rewards we start from current_skash, undo unstakes, undo stakes and finally subtract trip_start_skash
    // trip_rewards = current_skash + trip_accum_unstakes - trip_accum_stakes - trip_start_skash;
    /// trip_start: (timpestamp in nanoseconds) this field is set at account creation, so it will start metering rewards
    pub trip_start: Timestamp,
    /// How many skashs the user had at "trip_start". 
    pub trip_start_skash: u128,
    // how much the user staked since trip start. always incremented
    pub trip_accum_stakes: u128,
    // how much the user unstaked since trip start. always incremented
    pub trip_accum_unstakes: u128,

}

impl Default for Account {
    fn default() -> Self {
        Self {
            available: 0,
            stake_shares: 0,
            unstaked: 0,
            unstaked_requested_epoch_height: 0,
            //trip-meter
            trip_start: env::block_timestamp()/1_000_000, //converted from nanoseconds to miliseconds
            trip_start_skash:0,
            trip_accum_stakes:0,
            trip_accum_unstakes:0,
        }
    }
}

/// items in the Vec of staking pools
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq)]
pub struct StakingPoolInfo {

    account_id: AccountId,

    //if we've made an async call to this pool
    busy_lock: bool,

    //how much of the meta-pool must be staked in this pool
    //0=> do not stake, only unstake
    //100 => 1% , 250=>2.5%, etc. -- max: 10000=>100%
    weight_basis_points: u16,

    //total staked here
    staked: u128,

    //total unstaked in this pool
    unstaked: u128,
    
    //set when the unstake command is passed to the pool
    //waiting period is until env::EpochHeight == unstaked_requested_epoch_height+NUM_EPOCHS_TO_UNLOCK
    //We might have to block users from unstaking if all the pools are in a waiting period
    unstaked_requested_epoch_height: EpochHeight, // = env::epoch_height() + NUM_EPOCHS_TO_UNLOCK

    //EpochHeight where we asked the sp what were our staking rewards
    last_asked_rewards_epoch_height: EpochHeight,
    
}

//---------------------------
//  Main Contrac State    ---
//---------------------------
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct DiversifiedPool {
    
    /// Owner's account ID (it will be a DAO on phase II)
    pub owner_account_id: AccountId,
    /// owner_fee_basis_points. 100 basis point => 1%. E.g.: owner_fee_basis_points=50 => 0.5% owner's fee
    pub owner_fee_basis_points: u16,

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
    /// During heartbeat(), If !staking_paused && total_for_staking<total_actually_staked, then the difference gets staked in 100kN batches
    pub total_actually_staked: u128,
    // how many "shares" were minted. Everytime someone "stakes" he "buys pool shares" with the staked amount
    // the share price is computed so if he "sells" the shares on that moment he recovers the same near amount
    // staking produces rewards, so share_price = total_for_staking/total_shares
    // when someone "unstakes" she "burns" X shares at current price to recoup Y near
    pub total_stake_shares: u128,

    /// The total amount of tokens selected for unstaking by the users 
    /// not necessarily what's actually unstaked since unstaking is done in batches
    /// If a user ask unstaking 100: total_for_unstaking+=100, total_for_staking-=100, total_stake_shares-=share_amount
    pub total_for_unstaking: u128,
    /// The total amount of tokens actually unstaked (the tokens are in the staking pools)
    /// During heartbeat(), If !staking_paused && total_for_unstaking<total_actually_unstaked, then the difference gets unstaked in 100kN batches
    pub total_actually_unstaked: u128,
    /// The total amount of tokens actually unstaked AND retrieved from the pools (the tokens are here)
    /// During heartbeat(), If sp.pending_withdrawal && sp.epoch_for_withdraw == env::epoch_height then all funds are retrieved from the sp
    /// When the funds are actually withdraw by the users, total_actually_unstaked is decremented
    pub total_actually_unstaked_and_retrieved: u128,

    /// the staking pools will add rewards to the staked amount on each epoch
    /// here we store the accumulatred amount only for stats purposes. This amount can only grow
    pub accumulated_staked_rewards: u128, 

    /// no auto-staking. true while changing staking pools
    pub staking_paused: bool, 

    //user's accounts
    pub accounts: UnorderedMap<AccountId, Account>,

    //list of pools to diversify in
    pub staking_pools: Vec<StakingPoolInfo>, 

}

impl Default for DiversifiedPool {
    fn default() -> Self {
        env::panic(b"The contract is not initialized.");
    }
}

#[near_bindgen]
impl DiversifiedPool {

    /* NOTE
    This contract must implement several traits

    1. deposit-trait [NEP-xxx]: this contract implements: deposit, get_available_balance, withdraw, withdraw_all 
       A [NEP-xxx] contract creates an account on deposit and allows you to withdraw later under certain conditions, deletes the account on withdraw_all 

    2. staking-pool [NEP-xxx]: this contract must be perceived as a staking-pool for the lockup-contract, wallets, and users too if they want
      that means implmenting: ping, deposit, deposit_and_stake, withdraw_all, withdraw, stake_all, stake, unstake_all, unstake
        and view methods: get_account_unstaked_balance, get_account_staked_balance, get_account_total_balance, is_account_unstaked_balance_available,
            get_total_staked_balance, get_owner_id, get_reward_fee_fraction, is_staking_paused, get_staking_key, get_account,
            get_number_of_accounts, get_accounts. 

    3. diversified-staking: these are the extensions to the standard staking pool (buy/sell skash)

    */

    /// Requires 25 TGas (1 * BASE_GAS)
    ///
    /// Initializes DiversifiedPool contract.
    /// - `owner_account_id` - the account ID of the owner.  Only this account can call owner's methods on this contract.
    #[init]
    pub fn new( owner_account_id: AccountId ) -> Self {
        assert!(!env::state_exists(), "The contract is already initialized");
        assert!(
            env::is_valid_account_id(owner_account_id.as_bytes()),
            "The account ID of the owner is invalid"
        );

        return Self {
            owner_account_id,
            owner_fee_basis_points: DEFAULT_OWNER_FEE_BASIS_POINTS,
            staking_paused: true, //no auto-staking. on while there's no staking pool defined
            total_available: 0,
            total_for_staking: 0,
            total_for_unstaking: 0,
            total_actually_staked: 0, //amount actually sent to the staking_pool and staked
            total_actually_unstaked: 0, // tracks unstaked amount from the staking_pool (toekns are in the pool)
            total_actually_unstaked_and_retrieved: 0, // tracks unstaked AND retrieved amount (tokens are here)
            accumulated_staked_rewards: 0,
            total_stake_shares: 0,
            accounts: UnorderedMap::new("A".into()),
            staking_pools: Vec::new(), 
        }
    }


//------------------------------------
// deposit trait & staking-pool trait
//------------------------------------

    /// staking-pool's ping redirects to diversified-pool's heartbeat, Does a bit of work
    pub fn ping(&mut self) {
        self.heartbeat();
    }

    /// Deposits the attached amount into the inner account of the predecessor.
    #[payable]
    pub fn deposit(&mut self) {
        self.internal_deposit();
    }

    /// Withdraws from the availabe balance
    pub fn withdraw(&mut self, amount: U128String) {
        self.internal_withdraw(amount.into());
    }

    /// Withdraws ALL from the "availabe" balance
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
    /// staking-pool "unstaked" is equivalent to diversified-pool "availabe", but here
    /// we keep the staking-pool logic because we're implementing the staking-pool trait
    pub fn stake_all(&mut self) {
        let account_id = env::predecessor_account_id();
        let account = self.internal_get_account(&account_id);
        self.internal_stake(account.unstaked);
    }

    /// Stakes the given amount from the inner account of the predecessor.
    /// The inner account should have enough unstaked balance.
    pub fn stake(&mut self, amount: U128String) {
        let amount: u128 = amount.into();
        self.internal_stake(amount);
    }

    /// Unstakes all staked balance from the inner account of the predecessor.
    /// The new total unstaked balance will be available for withdrawal in four epochs.
    pub fn unstake_all(&mut self) {
        let account_id = env::predecessor_account_id();
        let account = self.internal_get_account(&account_id);
        let amount = self.amount_from_shares(account.stake_shares);
        self.inner_unstake(amount);
    }

    /// Unstakes the given amount from the inner account of the predecessor.
    /// The inner account should have enough staked balance.
    /// The new total unstaked balance will be available for withdrawal in four epochs.
    pub fn unstake(&mut self, amount: U128String) {
        let amount: u128 = amount.into();
        self.inner_unstake(amount);
    }

    /*****************************/
    /* staking-pool View methods */
    /*****************************/

    /// Returns the unstaked balance of the given account.
    pub fn get_account_unstaked_balance(&self, account_id: AccountId) -> U128String {
        //warning: self.get_account is public and gets HumanReadableAccount .- do not confuse with self.internal_get_account
        return self.get_account(account_id).unstaked_balance
    }

    /// Returns the staked balance of the given account.
    /// NOTE: This is computed from the amount of "stake" shares the given account has and the
    /// current amount of total staked balance and total stake shares on the account.
    pub fn get_account_staked_balance(&self, account_id: AccountId) -> U128String {
        //warning: self.get_account is public and gets HumanReadableAccount .- do not confuse with self.internal_get_account
        return self.get_account(account_id).staked_balance
    }

    /// Returns the total balance of the given account (including staked and unstaked balances).
    pub fn get_account_total_balance(&self, account_id: AccountId) -> U128String {
        //warning: self.get_account is public and gets HumanReadableAccount .- do not confuse with self.internal_get_account
        let account = self.get_account(account_id);
        return (account.unstaked_balance.0 + account.staked_balance.0).into()
    }

    /// Returns `true` if the given account can withdraw tokens in the current epoch.
    pub fn is_account_unstaked_balance_available(&self, account_id: AccountId) -> bool {
        //warning: self.get_account is public and gets HumanReadableAccount .- do not confuse with self.internal_get_account
        return self.get_account(account_id).can_withdraw
    }

    /// Returns account ID of the staking pool owner.
    pub fn get_owner_id(&self) -> AccountId {
        return self.owner_account_id.clone();
    }

    /// Returns the current reward fee as a fraction.
    pub fn get_reward_fee_fraction(&self) -> RewardFeeFraction {
        return RewardFeeFraction {
            numerator: self.owner_fee_basis_points.into(),
            denominator: 10_000
        };
    }

    /// Returns the staking public key
    pub fn get_staking_key(&self) -> Base58PublicKey {
        panic!("no specific staking key");
    }

    /// Returns true if the staking is paused
    pub fn is_staking_paused(&self) -> bool {
        return self.staking_paused;
    }

    /// to implement the Staking-pool inteface, get_account returns the same as the staking-pool returns
    /// full account info can be obtained by calling: pub fn get_account_info(&self, account_id: AccountId) -> GetAccountInfoResult 
    /// Returns human readable representation of the account for the given account ID.
    pub fn get_account(&self, account_id: AccountId) -> HumanReadableAccount {
        let account = self.internal_get_account(&account_id);
        return HumanReadableAccount {
            account_id,
            unstaked_balance: account.unstaked.into(),
            staked_balance: self
                .amount_from_shares(account.stake_shares)
                .into(),
            can_withdraw: env::epoch_height() >= account.unstaked_requested_epoch_height+NUM_EPOCHS_TO_UNLOCK,
        }
    }

    /// Returns the number of accounts that have positive balance on this staking pool.
    pub fn get_number_of_accounts(&self) -> u64 {
        return self.accounts.len()
    }

    /// Returns the list of accounts
    pub fn get_accounts(&self, from_index: u64, limit: u64) -> Vec<HumanReadableAccount> {

        let keys = self.accounts.keys_as_vector();

        //warning: self.get_account is public and gets HumanReadableAccount .- do not confuse with self.internal_get_account
        return (from_index..std::cmp::min(from_index + limit, keys.len()))
            .map(|index| self.get_account(keys.get(index).unwrap()))
            .collect()
    }

//----------------------------------
//----------------------------------
// DIVERISIFYING-STAKING-POOL trait
//----------------------------------
//----------------------------------

    /// user method
    /// completes unstake action by moving from retreieved_from_the_pools to availabe
    pub fn complete_unstaking(&mut self) {
        
        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);
        let amount = account.unstaked;
        assert!(
            amount > 0,
            "No unstaked balance"
        );
        let epoch = env::epoch_height();
        if  epoch < account.unstaked_requested_epoch_height+NUM_EPOCHS_TO_UNLOCK  {
            panic!(format!("The unstaked balance is not yet available due to unstaking delay. You need to wait {} epochs", 
                            account.unstaked_requested_epoch_height+NUM_EPOCHS_TO_UNLOCK - epoch).as_bytes());
        }

        //async: try to do one of the pending withdrawals
        self.internal_async_withdraw_from_a_pool();

        if self.total_actually_unstaked_and_retrieved < amount {
            panic!("Please wait one more hour until the funds are retrieved from the pools");
        }

        assert!(self.total_for_unstaking > amount);

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

    /// user method
    /// places a buy-skash order (stake)
    pub fn place_buy_skash_order(&mut self, amount:U128String, discount:i32, duration:i32) {
        
        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);
        let am:u128 = amount.0;
        assert!(
            account.availabe > 0,
            "Not enough available balance"
        );
        let epoch = env::epoch_height();
        if  epoch < account.unstaked_requested_epoch_height+NUM_EPOCHS_TO_UNLOCK  {
            panic!(format!("The unstaked balance is not yet available due to unstaking delay. You need to wait {} epochs", 
                            account.unstaked_requested_epoch_height+NUM_EPOCHS_TO_UNLOCK - epoch).as_bytes());
        }

        //async: try to do one of the pending withdrawals
        self.internal_async_withdraw_from_a_pool();

        if self.total_actually_unstaked_and_retrieved < amount {
            panic!("Please wait one more hour until the funds are retrieved from the pools");
        }

        assert!(self.total_for_unstaking > amount);

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



//-----------------------------
// HEARTBEAT
//-----------------------------

    /// operator method
    /// heartbeat. Do staking & unstaking in batches of at most 100Kn
    /// called externaly every 30 mins or less if: a) there's a large stake/unstake oper to perform or b) the epoch is about to finish and there are stakes to be made
    /// returns "true" if there's still more job to do
    pub fn heartbeat(&mut self) {

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

        //-------------------------------------
        //internal clearing, no need to talk to the pools
        //-------------------------------------
        if amount_to_stake>0 && amount_to_unstake>0 {
            if amount_to_stake > amount_to_unstake {
                amount_to_stake -= amount_to_unstake;
                amount_to_unstake = 0;
            }
            else {
                amount_to_unstake -= amount_to_stake ;
                amount_to_stake = 0;
            }
        }

        //-------------------------------------
        //check if we need to actually stake
        //-------------------------------------
        if amount_to_stake>0 {
            //more ordered for staking than actually staked
            // do it in batches of 100/150k
            if amount_to_stake > MAX_NEARS_SINGLE_MOVEMENT {
                //split movements
                amount_to_stake = NEARS_PER_BATCH;
            }
            let sp_inx = self.get_staking_pool_requiring_stake();
            if sp_inx!=usize::MAX {
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
                    assert!(sp.unstaked>=amount_to_stake);
                    self.total_actually_staked += amount_to_stake; //preventively consider the amount staked (undoes if failed)
                    ext_staking_pool::stake(
                        amount_to_stake.into(),
                        &sp.account_id,
                        NO_DEPOSIT,
                        gas::staking_pool::STAKE,
                    )
                    .then(ext_self_owner::on_staking_pool_stake_maybe_deposit(
                        sp_inx, amount_to_stake, false,
                        &env::current_account_id(),
                        NO_DEPOSIT,
                        gas::owner_callbacks::ON_STAKING_POOL_DEPOSIT_AND_STAKE,
                    ));

                    return; //just one bit of work
                }

                //here the sp has no unstaked balance, we must deposit_and_stake on the sp
                //launch async deposit_and_stake on the pool
                assert!( env::account_balance()-MIN_BALANCE_FOR_STORAGE >= amount_to_stake, "env::account_balance()-MIN_BALANCE_FOR_STORAGE < amount_to_deposit_and_stake");
                assert!( self.total_available >= amount_to_stake, "self.available < amount_to_deposit_and_stake");
                self.total_available -= amount_to_stake;//preventively consider the amount sent (undoes if async fails)
                self.total_actually_staked += amount_to_stake; //preventively consider the amount staked (undoes if async fails)

                ext_staking_pool::deposit_and_stake(
                    &sp.account_id,
                    amount_to_stake.into(), //attached amount
                    gas::staking_pool::DEPOSIT_AND_STAKE,
                )
                .then(ext_self_owner::on_staking_pool_stake_maybe_deposit(
                    sp_inx, amount_to_stake, true,
                    &env::current_account_id(),
                    NO_DEPOSIT,
                    gas::owner_callbacks::ON_STAKING_POOL_DEPOSIT_AND_STAKE,
                ));

                return; //just one bit of work
                        
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
            if sp_inx!=usize::MAX {
                //most unbalanced pool found & available
                //launch async to unstake

                let sp = &mut self.staking_pools[sp_inx];
                sp.busy_lock = true;

                //max to unstake is amount staked
                if sp.staked < amount_to_unstake {
                    amount_to_unstake = sp.staked;
                }
                //launch async to un-stake from the pool
                assert!(sp.staked>=amount_to_unstake);
                self.total_actually_staked -= amount_to_unstake; //preventively consider the amount un-staked (undoes if promise fails)
                self.total_actually_unstaked += amount_to_unstake; //preventively consider the amount un-staked (undoes if promise fails)
                ext_staking_pool::unstake(
                    amount_to_unstake.into(),
                    &sp.account_id,
                    NO_DEPOSIT,
                    gas::staking_pool::UNSTAKE,
                )
                .then(ext_self_owner::on_staking_pool_unstake(
                    sp_inx, amount_to_unstake, 
                    &env::current_account_id(),
                    NO_DEPOSIT,
                    gas::owner_callbacks::ON_STAKING_POOL_UNSTAKE,
                ));

                return; //just one bit of work
                        
            }

        }

    }

    //prev fn continues here
    /// Called after amount is staked from the sp's unstaked balance (all into  the staking pool contract).
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_stake_maybe_deposit(&mut self, sp_inx:usize, amount: u128, included_deposit:bool) -> bool {
        assert_self();

        let sp = &mut self.staking_pools[sp_inx];
        sp.busy_lock = false;
    
        let stake_succeeded = is_promise_success();

        let result:&str;
        if stake_succeeded {
            result="succeeded";
            if !included_deposit {
                //not deposited first, so staked funds came from unstaked funds already in the sp 
                sp.unstaked -= amount;
            }
            //move into staked
            sp.staked += amount;
        } 
        else {
            result="has failed";
            if included_deposit {
                self.total_available += amount; //undo preventive action considering the amount sent
            }
            self.total_actually_staked -= amount; //undo preventive action considering the amount staked
        }
        env::log(
            format!(
                "Staking of {} at @{} {}",
                amount,
                sp.account_id,
                result
            )
            .as_bytes(),
        );
        return stake_succeeded
    }

    /// Called after the given amount was unstaked at the staking pool contract.
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_unstake(&mut self, sp_inx:usize, amount: u128) -> bool {
        assert_self();

        let sp = &mut self.staking_pools[sp_inx];
        sp.busy_lock = false;

        let unstake_succeeded = is_promise_success();

        let result:&str;
        if unstake_succeeded {
            result="succeeded";
            sp.unstaked+=amount;
            sp.staked-=amount;
        } else {
            result="has failed";
            self.total_actually_staked += amount; //undo preventive action considering the amount unstaked
            self.total_actually_unstaked -= amount; //undo preventive action considering the amount unstaked
        }

        env::log(
            format!(
                "Unstaking of {} at @{} {}",
                amount,
                sp.account_id,
                result
            )
            .as_bytes(),
        );
        return unstake_succeeded;
    }


//------------------------------------------
// GETTERS (moved from getters.rs)
//------------------------------------------
    /// Returns the account ID of the owner.
    pub fn get_owner_account_id(&self) -> AccountId {
        return self.owner_account_id.clone()
    }

    /// The amount of tokens that were deposited to the staking pool.
    /// NOTE: The actual balance can be larger than this known deposit balance due to staking
    /// rewards acquired on the staking pool.
    /// To refresh the amount the owner can call `refresh_staking_pool_balance`.
    pub fn get_known_deposited_balance(&self) -> U128String {
        return self.total_actually_staked.into()
    }

    /// full account info
    /// Returns JSON representation of the account for the given account ID.
    pub fn get_account_info(&self, account_id: AccountId) -> GetAccountInfoResult {
        let account = self.internal_get_account(&account_id);
        let skash = self.amount_from_shares(account.stake_shares);
        // trip_rewards = current_skash + trip_accum_unstakes - trip_accum_stakes - trip_start_skash;
        let trip_rewards = (skash + account.trip_accum_unstakes).saturating_sub(account.trip_accum_stakes+account.trip_start_skash);
        return GetAccountInfoResult {
            account_id,
            available: account.available.into(),
            skash: skash.into(),
            unstaked: account.unstaked.into(),
            unstaked_requested_epoch_height: account.unstaked_requested_epoch_height.into(),
            can_withdraw: (env::epoch_height()>=account.unstaked_requested_epoch_height+NUM_EPOCHS_TO_UNLOCK),
            total: (account.available + skash + account.unstaked).into(),
            //trip-meter
            trip_start: account.trip_start.into(),
            trip_start_skash: account.trip_start_skash.into(),
            trip_accum_stakes: account.trip_accum_stakes.into(),
            trip_accum_unstakes: account.trip_accum_unstakes.into(),
            trip_rewards: trip_rewards.into()
        }
    }

    /// get contract totals info
    /// Returns JSON representation of the totals
    pub fn get_contract_info(&self) -> GetContractInfoResult {
        return GetContractInfoResult {
            owner_account_id: self.owner_account_id.clone(), 
            owner_fee_basis_points: self.owner_fee_basis_points,
            total_available: self.total_available.into(),
            total_for_staking: self.total_for_staking.into(),
            total_for_unstaking: self.total_for_unstaking.into(),
            total_actually_staked: self.total_actually_staked.into(),
            accumulated_staked_rewards: self.accumulated_staked_rewards.into(),
            total_actually_unstaked: self.total_actually_unstaked.into(),
            total_actually_unstaked_and_retrieved: self.total_actually_unstaked_and_retrieved.into(),
            staking_paused: self.staking_paused,
            total_stake_shares: self.total_stake_shares.into(),
            accounts_count: self.accounts.len().into(),
            staking_pools_count: (self.staking_pools.len() as u64).into(),
        }
    }

    /// get sp (staking-pool) info
    /// Returns JSON representation of sp recorded state
    pub fn get_sp_info(&self, sp_inx_i32:i32) -> GetSpInfoResult {

        assert!(sp_inx_i32>0);

        self.assert_owner();

        let sp_inx = sp_inx_i32 as usize;
        assert!(sp_inx < self.staking_pools.len());

        let sp = &self.staking_pools[sp_inx];

        return GetSpInfoResult {
            account_id: sp.account_id.clone(), 
            weight_basis_points: sp.weight_basis_points,
            staked: sp.staked.into(),
            unstaked: sp.unstaked.into(),
            unstaked_requested_epoch_height: sp.unstaked_requested_epoch_height.into(),
            last_asked_rewards_epoch_height: sp.last_asked_rewards_epoch_height.into(),
        }
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
            to_yocto(gfme_NEAR),
            0,
            to_ts(GENESIS_TIME_IN_DAYS),
            false,
        )
    }


    fn new_contract() -> DiversifiedPool {
        DiversifiedPool::new(
            account_owner()
        )
    }

    fn gfme_only_setup() -> (VMContext, DiversifiedPool) {
        let context = basic_context();
        testing_env!(context.clone());
        let contract = new_contract();
        return (context, contract)
    }

    // #[test]
    // fn test_gfme_only_basic() {
    //     let (mut context, contract) = gfme_only_setup();
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

    //     assert_almost_eq(contract.get_owners_balance().0, to_yocto(gfme_NEAR));
    // }

    #[test]
    fn test_add_full_access_key() {
        let (mut context, mut contract) = gfme_only_setup();
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
        let (mut context, mut contract) = gfme_only_setup();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR);
        context.predecessor_account_id = non_owner();
        context.signer_account_id = non_owner();
        testing_env!(context.clone());

        contract.select_staking_pool(AccountId::from("staking_pool"));
    }


    #[test]
    fn test_gfme_only_transfer_call_by_owner() {
        let (mut context, mut contract) = gfme_only_setup();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
        context.is_view = true;
        testing_env!(context.clone());
        assert_almost_eq(contract.get_owners_balance().0, to_yocto(gfme_NEAR));

        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(1).try_into().unwrap();
        context.is_view = false;
        testing_env!(context.clone());

        assert_eq!(env::account_balance(), to_yocto(gfme_NEAR));
        contract.transfer(to_yocto(100).into(), non_owner());
        assert_almost_eq(env::account_balance(), to_yocto(gfme_NEAR - 100));
    }

    #[test]
    #[should_panic(expected = "Staking pool is not selected")]
    fn test_staking_pool_is_not_selected() {
        let (mut context, mut contract) = gfme_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        let amount = to_yocto(gfme_NEAR - 100);
        testing_env!(context.clone());
        contract.deposit_to_staking_pool(amount.into());
    }

    #[test]
    fn test_staking_pool_success() {
        let (mut context, mut contract) = gfme_only_setup();
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
        let amount = to_yocto(gfme_NEAR - 100);
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.deposit_to_staking_pool(amount.into());
        context.account_balance = env::account_balance();
        assert_eq!(context.account_balance, to_yocto(gfme_NEAR) - amount);

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
        let (mut context, mut contract) = gfme_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        // Selecting staking pool
        let staking_pool = "staking_pool".to_string();
        testing_env!(context.clone());
        contract.select_staking_pool(staking_pool.clone());

        // Deposit to the staking_pool
        let amount = to_yocto(gfme_NEAR - 100);
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.deposit_to_staking_pool(amount.into());
        context.account_balance = env::account_balance();
        assert_eq!(context.account_balance, to_yocto(gfme_NEAR) - amount);

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
        let (mut context, mut contract) = gfme_only_setup();
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
        let (mut context, mut contract) = gfme_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        // Unselecting staking pool
        testing_env!(context.clone());
        contract.unselect_staking_pool();
    }


    #[test]
    fn test_staking_pool_owner_balance() {
        let (mut context, mut contract) = gfme_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);

        let gfme_amount = to_yocto(gfme_NEAR);
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
}

