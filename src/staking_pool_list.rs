use near_sdk::{env,AccountId};
use near_sdk::borsh::{self,BorshDeserialize, BorshSerialize};
pub use crate::types::*;

const ERR_ELEMENT_SERIALIZATION: &[u8] = b"Cannot serialize element";


//list of pools to diversify in
type StakingPoolsBTreeMap = std::collections::BTreeMap<String,StakingPoolInfo>;
/// items in the Vec of staking pools
#[derive(Default)]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakingPoolInfo {
    pub account_id: AccountId,

    //if we've made an async call to this pool
    pub busy_lock: bool,

    //how much of the meta-pool must be staked in this pool
    //0=> do not stake, only unstake
    //100 => 1% , 250=>2.5%, etc. -- max: 10000=>100%
    pub weight_basis_points: u16,

    //total staked here
    pub staked: u128,

    //total unstaked in this pool
    pub unstaked: u128,
    //set when the unstake command is passed to the pool
    //waiting period is until env::EpochHeight == unstaked_requested_epoch_height+NUM_EPOCHS_TO_UNLOCK
    //We might have to block users from unstaking if all the pools are in a waiting period
    pub unstaked_requested_epoch_height: EpochHeight, // = env::epoch_height() + NUM_EPOCHS_TO_UNLOCK

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
}

//---------------------------------
// staking-pools-list (SPL) management
//---------------------------------
pub(crate) fn read_from_storage() -> StakingPoolsBTreeMap {

    let raw_data = match env::storage_read("SPL".as_bytes()) {
        Some(x) => x,
        None => env::panic("Not initialized".as_bytes()),
    };

    return match StakingPoolsBTreeMap::try_from_slice(&raw_data) {
        Ok(x) => x,
        Err(_) => env::panic("EES".as_bytes()),
    };
}

pub(crate) fn save_to_storage(map: &StakingPoolsBTreeMap) {

    let raw_data = match map.try_to_vec() {
        Ok(x) => x,
        Err(_) => env::panic(ERR_ELEMENT_SERIALIZATION),
    };

    env::storage_write("SPL".as_bytes(), &raw_data);
}
