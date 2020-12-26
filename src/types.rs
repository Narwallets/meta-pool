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
pub const TWO_NEAR: u128 = 2 * ONE_NEAR;
pub const FIVE_NEAR: u128 = 5 * ONE_NEAR;
pub const TEN_NEAR: u128 = 10 * ONE_NEAR;
pub const NEAR_100K: u128 = 100_000 * ONE_NEAR;
pub const NEARS_PER_BATCH: u128 = NEAR_100K; // if amount>MAX_NEARS_SINGLE_MOVEMENT then it's splited in NEARS_PER_BATCH batches
pub const MAX_NEARS_SINGLE_MOVEMENT: u128 = NEARS_PER_BATCH + NEARS_PER_BATCH/2; //150K max movement, if you try to stake 151K, it will be split into 2 movs, 100K and 51K

pub const NUM_EPOCHS_TO_UNLOCK: EpochHeight = 0; //0 for testing in guidlnet, 4 for mainnet & testnet;

//cut on swap fees
pub const DEFAULT_TREASURY_SWAP_CUT_BASIS_POINTS : u16 = 2500; // 25% swap fees go to Treasury
pub const DEFAULT_OPERATOR_SWAP_CUT_BASIS_POINTS : u16 = 300; // 3% swap fees go to operator
//Fee on staking rewards
pub const DEFAULT_OPERATOR_REWARDS_FEE_BASIS_POINTS : u16 = 50; // 0.5% -- CANT BE HIGER THAN 1000 / 10%

//Note: Licence forbids you to change the following 3 constants and/or the developer's distribution mechanism
pub const DEVELOPERS_ACCOUNT_ID: &str = "developers.near"; 
pub const DEVELOPERS_REWARDS_FEE_BASIS_POINTS: u16 = 20; // 0.2% from rewards 
pub const DEVELOPERS_SWAP_CUT_BASIS_POINTS : u16 = 200; // 2% swap fees go to authors


construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}

/// Raw type for duration in nanoseconds
pub type Duration = u64;
/// Raw type for timestamp in nanoseconds or Unix Ts in miliseconds
pub type Timestamp = u64;

/// Balance wrapped into a struct for JSON serialization as a string.
pub type U128String = U128;
pub type U64String = U64;

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
    /// The amount unstaked waiting for withdraw
    pub unstaked: U128,
    /// The epoch height when the unstaked was requested
    /// The fund will be locked for NUM_EPOCHS_TO_UNLOCK epochs
    /// unlock epoch = unstaked_requested_epoch_height + NUM_EPOCHS_TO_UNLOCK 
    pub unstaked_requested_epoch_height: U64,
    ///if env::epoch_height()>=account.unstaked_requested_epoch_height+NUM_EPOCHS_TO_UNLOCK
    pub can_withdraw: bool,
    /// total amount the user holds in this contract: account.available + account.staked + current_rewards + account.unstaked
    pub total: U128,

    //-- STATISTICAL DATA --
    // User's statistical data
    // These fields works as a car's "trip meter". The user can reset them to zero.
    /// trip_start: (timpestamp in nanoseconds) this field is set at account creation, so it will start metering rewards
    pub trip_start: U64,
    /// How many skashs the user had at "trip_start". 
    pub trip_start_skash: U128,
    /// how much the user staked since trip start. always incremented
    pub trip_accum_stakes: U128,
    /// how much the user unstaked since trip start. always incremented
    pub trip_accum_unstakes: U128,
    /// to compute trip_rewards we start from current_skash, undo unstakes, undo stakes and finally subtract trip_start_skash
    /// trip_rewards = current_skash + trip_accum_unstakes - trip_accum_stakes - trip_start_skash;
    /// trip_rewards = current_skash + trip_accum_unstakes - trip_accum_stakes - trip_start_skash;
    pub trip_rewards: U128,

    //NLSP
    pub nslp_shares: U128,
    pub nslp_share_value: U128,

    //G-SKASH owned (including pending rewards)
    pub g_skash: U128,

}


/// Struct returned from get_contract_state
/// div-pool state info
/// Represents contact state as as JSON compatible struct
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct GetContractStateResult {
    /// This amount increments with deposits and decrements when users stake
    /// increments with complete_unstake and decrements with user withdrawals from the contract
    /// withdrawals from the pools can include rewards
    /// since staking is delayed and in batches it only eventually matches env::balance()
    pub total_available: U128,

    /// The total amount of tokens selected for staking by the users 
    /// not necessarily what's actually staked since staking can is done in batches
    /// Share price is computed using this number. share_price = total_for_staking/total_shares
    pub total_for_staking: U128,
    /// The total amount of tokens actually staked (the tokens are in the staking pools)
    /// During heartbeat(), If !staking_paused && total_for_staking<total_actually_staked, then the difference gets unstaked in 100kN batches
    pub total_actually_staked: U128,
    // how many "shares" were minted. Everytime someone "stakes" he "buys pool shares" with the staked amount
    // the share price is computed so if he "sells" the shares on that moment he recovers the same near amount
    // staking produces rewards, so share_price = total_for_staking/total_shares
    // when someone "unstakes" she "burns" X shares at current price to recoup Y near
    pub total_stake_shares: U128,

    /// The total amount of tokens selected for unstaking by the users 
    /// not necessarily what's actually unstaked since unstaking is done in batches
    /// If a user ask unstaking 100: total_for_unstaking+=100, total_for_staking-=100, total_stake_shares-=share_amount
    pub total_for_unstaking: U128,
    /// The total amount of tokens actually unstaked (the tokens are in the staking pools)
    /// During heartbeat(), If !staking_paused && total_for_unstaking<total_actually_unstaked, then the difference gets unstaked in 100kN batches
    pub total_actually_unstaked: U128,
    /// The total amount of tokens actually unstaked AND retrieved from the pools (the tokens are here)
    /// During heartbeat(), If sp.pending_withdrawal && sp.epoch_for_withdraw == env::epoch_height then all funds are retrieved from the sp
    /// When the funds are actually withdraw by the users, total_actually_unstaked is decremented
    pub total_actually_unstaked_and_retrieved: U128,

    /// total g-skash minted
    pub total_g_skash: U128, 

    /// the staking pools will add rewards to the staked amount on each epoch
    /// here we store the accumulatred amount only for stats purposes. This amount can only grow
    pub accumulated_staked_rewards: U128, 

    //how many accounts there are
    pub accounts_count: U64,

    //count of pools to diversify in
    pub staking_pools_count: U64, 

}

/// Struct returned from get_contract_params
/// div-pool parameters info
/// Represents contact parameters as JSON compatible struct
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct ContractParamsJSON {

    /// no auto-staking. true while changing staking pools
    pub staking_paused: bool, 

    /// adjustable min account available balance (to backup storage)
    pub min_account_balance: U128String,

    ///NEAR/SKASH Liquidity pool target
    pub nslp_near_target: U128String,
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

    /// operator_fee_basis_points. 100 basis point => 1%. E.g.: owner_fee_basis_points=50 => 0.5% owner's fee
    pub operator_rewards_fee_basis_points: u16,
    /// operator_cut_basis_points. 
    pub operator_swap_cut_basis_points: u16,
    /// treasury_cut_basis_points. 
    pub treasury_swap_cut_basis_points: u16,
    
}

// get_staking_pools_list returns StakingPoolJSONInfo[]
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolJSONInfo {
    pub account_id: String,
    pub weight_basis_points: u16,
    pub staked: U128String,
    pub unstaked: U128String,
    pub unstaked_requested_epoch_height: U64String, 
    //EpochHeight where we asked the sp what were our staking rewards
    pub last_asked_rewards_epoch_height: U64String,
}

