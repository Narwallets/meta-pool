use crate::*;

pub use crate::types::*;
pub use crate::utils::*;

// ------------------
// Staking Pools Data
// ------------------

//-------------------------
//--  STAKING POOL Info  --
//-------------------------
/// items in the Vec of staking pools
#[derive(Default, BorshDeserialize, BorshSerialize)]
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
            && self.unstaked == 0;
    }
    pub fn new(account_id: AccountId, weight_basis_points: u16) -> Self {
        return Self {
            account_id,
            weight_basis_points,
            busy_lock: false,
            staked: 0,
            unstaked: 0,
            unstk_req_epoch_height: 0,
            last_asked_rewards_epoch_height: 0,
        };
    }
    pub fn total_balance(&self) -> u128 {
        self.staked + self.unstaked
    }

    pub fn wait_period_ended(&self) -> bool {
        let epoch_height = env::epoch_height();
        if self.unstk_req_epoch_height > epoch_height {
            //bad data at unstk_req_epoch_height or there was a hard-fork
            return true;
        }
        //true if we reached epoch_requested+NUM_EPOCHS_TO_UNLOCK
        return epoch_height >= self.unstk_req_epoch_height + NUM_EPOCHS_TO_UNLOCK;
    }
}

// -------------------
// Staking Pools Trait
// -------------------
#[ext_contract(ext_staking_pool)]
pub trait ExtStakingPool {
    fn get_account_staked_balance(&self, account_id: AccountId) -> U128String;

    fn get_account_unstaked_balance(&self, account_id: AccountId) -> U128String;

    fn get_account_total_balance(&self, account_id: AccountId) -> U128String;

    fn deposit(&mut self);

    fn deposit_and_stake(&mut self);

    fn withdraw(&mut self, amount: U128String);
    fn withdraw_all(&mut self);

    fn stake(&mut self, amount: U128String);

    fn unstake(&mut self, amount: U128String);

    fn unstake_all(&mut self);
}
