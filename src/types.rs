use near_sdk::json_types::{U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{AccountId};
use uint::construct_uint;

/// The contract keeps at least 35 NEAR in the account to avoid being transferred out to cover
/// contract code storage and some internal state.
pub const MIN_BALANCE_FOR_STORAGE: u128 = 35_000_000_000_000_000_000_000_000;

/// useful constants
pub const NO_DEPOSIT: u128 = 0;
pub const ONE_NEAR: u128 = 1_000_000_000_000_000_000_000_000;
pub const TEN_NEAR: u128 = 10 * ONE_NEAR;
pub const NEAR_100K: u128 = 100_000 * ONE_NEAR;
pub const NEARS_PER_BATCH: u128 = NEAR_100K; // if amount>MAX_NEARS_SINGLE_MOVEMENT then it's splited in NEARS_PER_BATCH batches
pub const MAX_NEARS_SINGLE_MOVEMENT: u128 = NEARS_PER_BATCH + NEARS_PER_BATCH/2; //150K max movement, if you try to stake 151K, it will be split into 2 movs, 100K and 51K

pub const NUM_EPOCHS_TO_UNLOCK: EpochHeight = 4;

pub const DEFAULT_ONWER_FEE_BASIS_POINTS : u16 = 50; // 0.5% -- CANT BE HIGER THAN 10_000 / 100%
//Note: Author's Licence forbids you to change the following 3 constants and/or the author's distribution mechanism
pub const AUTHOR_ACCOUNT_ID: &str = "developers.near"; 
pub const AUTHOR_MIN_FEE_BASIS_POINTS: u16 = 25; // 0.25% of the benefits -- CANT BE HIGER THAN 5_000 / 50%
pub const AUTHOR_MIN_FEE_OPERATOR_BP: u16 = 200; // or 2% of the owner's fee


construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}

/// Raw type for duration in nanoseconds
pub type Duration = u64;
/// Raw type for timestamp in nanoseconds
pub type Timestamp = u64;

/// Timestamp in nanosecond wrapped into a struct for JSON serialization as a string.
pub type WrappedTimestamp = U64;
/// Balance wrapped into a struct for JSON serialization as a string.
pub type U128String = U128;

pub type EpochHeight = u64;

/// Hash of Vesting schedule.
pub type Hash = Vec<u8>;

/// Rewards fee fraction structure for the staking pool contract.
#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct RewardFeeFraction {
    pub numerator: u32,
    pub denominator: u32,
}

/// staking-pool trait
/// Represents an account structure readable by humans.
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct HumanReadableAccount {
    pub account_id: AccountId,
    /// The unstaked balance that can be withdrawn or staked.
    pub unstaked_balance: U128,
    /// The amount balance staked at the current "stake" share price.
    pub staked_balance: U128,
    /// Whether the unstaked balance is available for withdrawal now.
    pub can_withdraw: bool,
}

/// Struct returned from get_account_info
/// div-pool full info
/// Represents account data as as JSON compatible struct
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct GetAccountInfoResult {
    pub account_id: AccountId,
    /// The available balance that can be withdrawn
    pub available: U128,
    /// The amount of SKASH owned (computed from the shares owned)
    pub skash: U128,
    /// The amount of rewards (rewards = total_staked - skash_amount) and (total_owned = skash + rewards)
    pub rewards: U128,
    /// Accumulated rewards during the lifetime of this account. 
    pub historic_rewards: U128,
    /// The amount unstaked waiting for withdraw
    pub unstaked: U128,
    /// The epoch height when the unstaked was requested
    /// The fund will be locked for NUM_EPOCHS_TO_UNLOCK epochs
    /// unlock epoch = unstaked_requested_epoch_height + NUM_EPOCHS_TO_UNLOCK 
    pub unstaked_requested_epoch_height: U64,
    ///if env::epoch_height()>=account.unstaked_requested_epoch_height+NUM_EPOCHS_TO_UNLOCK
    pub can_withdraw: bool,
    /// total amount the user holds in this contract: account.availabe + account.staked + current_rewards + account.unstaked
    pub total: U128,
}
