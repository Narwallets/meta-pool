use crate::*;
use near_sdk::log;

pub use crate::types::*;
pub use crate::utils::*;

// -----------------
// User Account Data
// -----------------
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq)]
pub struct Account {
    /// This amount increments with deposits and decrements with for_staking
    /// increments with complete_unstake and decrements with user withdrawals from the contract
    /// withdrawals from the pools can include rewards
    /// since staking is delayed and in batches it only eventually matches env::balance()
    /// total = available + staked + unstaked
    /// Note: In the simplified user-UI, the basic-user always does deposit-and-stake and sell/unstake that goes directly to their wallet
    /// so the only users of this field are lockup-contracts and advanced-users when they perform "Classic Unstakes"
    pub available: u128,

    /// The amount of st_near (stake shares) of the total staked balance in the pool(s) this user owns.
    /// When someone stakes, share-price is computed and shares are "sold" to the user so he only owns what he's staking and no rewards yet
    /// When a user request a transfer to other user, shares from the origin are moved to shares of the destination
    /// The share_price can be computed as total_for_staking/total_stake_shares
    /// stNEAR price = total_for_staking/total_stake_shares
    pub stake_shares: u128, //st_near this account owns

    /// Incremented when the user asks for Delayed-Unstaking. The amount of unstaked near in the pools
    pub unstaked: u128,

    /// The epoch height when the unstaked will be available
    /// The funds will be locked for -AT LEAST- NUM_EPOCHS_TO_UNLOCK epochs
    pub unstaked_requested_unlock_epoch: EpochHeight,

    //-- META
    ///realized META, can be used to transfer META from one user to another
    // Total META = realized_meta + staking_meter.mul_rewards(valued_stake_shares) + lp_meter.mul_rewards(valued_lp_shares)
    // Every time the user operates on STAKE/UNSTAKE: we realize meta: realized_meta += staking_meter.mul_rewards(valued_staked_shares)
    // Every time the user operates on ADD.LIQ/REM.LIQ.: we realize meta: realized_meta += lp_meter.mul_rewards(valued_lp_shares)
    // if the user calls farm_meta() we perform both
    pub realized_meta: u128,
    ///Staking rewards meter (to mint stNEAR for the user)
    pub staking_meter: RewardMeter,
    ///LP fee gains meter (to mint meta for the user)
    pub lp_meter: RewardMeter,

    //-- STATISTICAL DATA --
    // User's statistical data
    // This is the user-controlled staking rewards meter, it works as a car's "trip meter". The user can reset them to zero.
    // to compute trip_rewards we start from current_stnear, undo unstakes, undo stakes and finally subtract trip_start_stnear
    // trip_rewards = current_stnear + trip_accum_unstakes - trip_accum_stakes - trip_start_stnear;
    /// trip_start: (timestamp in milliseconds) this field is set at account creation, so it will start metering rewards
    pub trip_start: Timestamp,

    /// How much stnear the user had at "trip_start".
    pub trip_start_stnear: u128,
    // how much stnear the staked since trip start (minus unstaked)
    pub trip_accum_stakes: u128,
    // how much the user unstaked since trip start (zeroed if there was stake)
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
            unstaked_requested_unlock_epoch: 0,
            //meta & reward-meters
            realized_meta: 0,
            staking_meter: RewardMeter::default(),
            lp_meter: RewardMeter::default(),
            //trip-meter fields
            trip_start: env::block_timestamp() / 1_000_000, //converted from nanoseconds to milliseconds
            trip_start_stnear: 0,
            trip_accum_stakes: 0,
            trip_accum_unstakes: 0,
            //NS liquidity pool
            nslp_shares: 0,
        }
    }
}
impl Account {
    /// when the account.is_empty() it will be removed
    pub fn is_empty(&self) -> bool {
        return self.available == 0
            && self.unstaked == 0
            && self.stake_shares == 0
            && self.nslp_shares == 0
            && self.realized_meta == 0;
    }

    #[inline]
    pub fn valued_nslp_shares(&self, main: &MetaPool, nslp_account: &Account) -> u128 {
        main.amount_from_nslp_shares(self.nslp_shares, &nslp_account)
    }

    /// return realized meta plus pending rewards
    pub fn total_meta(&self, main: &MetaPool) -> u128 {
        let valued_stake_shares = main.amount_from_stake_shares(self.stake_shares);
        let nslp_account = main.internal_get_nslp_account();
        let valued_lp_shares = self.valued_nslp_shares(main, &nslp_account);
        //debug!("self.realized_meta:{}, self.staking_meter.compute_rewards(valued_stake_shares):{} self.lp_meter.compute_rewards(valued_lp_shares):{}",
        //    self.realized_meta, self.staking_meter.compute_rewards(valued_stake_shares), self.lp_meter.compute_rewards(valued_lp_shares));
        return self.realized_meta
            + self.staking_meter.compute_rewards(valued_stake_shares, main.est_meta_rewards_stakers, main.max_meta_rewards_stakers)
            + self.lp_meter.compute_rewards(valued_lp_shares, main.est_meta_rewards_lp, main.max_meta_rewards_lp);
    }

    //---------------------------------
    /// realize meta from staking rewards
    pub fn stake_realize_meta(&mut self, main: &mut MetaPool) {
        //realize meta pending rewards on LP operation
        let valued_actual_shares = main.amount_from_stake_shares(self.stake_shares);
        let pending_meta = self
            .staking_meter
            .realize(valued_actual_shares, main.staker_meta_mult_pct, main.est_meta_rewards_stakers, main.max_meta_rewards_stakers);
        self.realized_meta += pending_meta;
        main.total_meta += pending_meta;
    }

    /// realize meta from nslp fees
    pub fn nslp_realize_meta(&mut self, nslp_account: &Account, main: &mut MetaPool) {
        //realize meta pending rewards on LP operation
        let valued_actual_shares = self.valued_nslp_shares(main, &nslp_account);
        let pending_meta = self
            .lp_meter
            .realize(valued_actual_shares, main.lp_provider_meta_mult_pct, main.est_meta_rewards_lp, main.max_meta_rewards_lp);
        self.realized_meta += pending_meta;
        main.total_meta += pending_meta;
    }

    //----------------
    // add st_nears, considering it as "a stake" for trip-meter purposes
    pub fn add_st_near(&mut self, st_near_amount: u128, main: &MetaPool) {
        self.add_stake_shares(
            st_near_amount,
            main.amount_from_stake_shares(st_near_amount),
        )
    }
    pub fn add_stake_shares(&mut self, num_shares: u128, near_amount: u128) {
        self.stake_shares += num_shares;
        //to buy stnear is stake
        self.trip_accum_stakes += near_amount;
        self.staking_meter.stake(near_amount);
    }

    // remove st_near considering is "an unstake" for trip-meter purposes
    pub fn sub_st_near(&mut self, st_near_amount: u128, main: &MetaPool) {
        self.sub_stake_shares(
            st_near_amount,
            main.amount_from_stake_shares(st_near_amount),
        )
    }
    pub fn sub_stake_shares(&mut self, num_shares: u128, near_amount: u128) {
        assert!(
            self.stake_shares >= num_shares,
            "sub_stake_shares self.stake_shares {} < num_shares {}",
            self.stake_shares,
            num_shares
        );
        self.stake_shares -= num_shares;
        //to sell stnear is to unstake
        self.trip_accum_unstakes += near_amount;
        if self.trip_accum_unstakes < self.trip_accum_stakes {
            //keep just the delta
            self.trip_accum_stakes -= self.trip_accum_unstakes;
            self.trip_accum_unstakes = 0;
        }
        self.staking_meter.unstake(near_amount);
    }

    /// user method
    /// completes unstake action by moving from acc.unstaked & main.reserve_for_unstaked_claims -> acc.available & main.total_available
    pub fn in_memory_try_finish_unstaking(
        &mut self,
        account_id: &str,
        amount: u128,
        main: &mut MetaPool,
    ) -> u128 {
        assert!(
            amount <= self.unstaked,
            "Not enough unstaked balance {}",
            self.unstaked
        );

        let epoch = env::epoch_height();
        assert!( epoch >= self.unstaked_requested_unlock_epoch,
            "The unstaked balance is not yet available due to unstaking delay. You need to wait at least {} epochs"
            , self.unstaked_requested_unlock_epoch - epoch);

        // in the account, moves from unstaked to available
        self.unstaked -= amount; //Zeroes, claimed
        self.available += amount;
        //check the heart beat has really moved the funds
        assert!(
            main.reserve_for_unstake_claims >= amount,
            "Funds are not yet available due to unstaking delay. Epoch:{}",
            env::epoch_height()
        );
        // in the contract, move from reserve_for_unstaked_claims to total_available
        main.reserve_for_unstake_claims -= amount;
        assert!(main.total_unstake_claims >= amount, "ITUC");
        main.total_unstake_claims -= amount;
        main.total_available += amount;

        event!(
            r#"{{"event":"D-WITHD","account_id":"{}","amount":"{}"}}"#,
            account_id,
            amount
        );

        log!("{} unstaked moved to available", amount);

        return amount;
    }

    pub(crate) fn take_from_available(
        &mut self,
        amount_requested: u128,
        main: &mut MetaPool,
    ) -> u128 {
        let to_withdraw:u128 =
        // if the amount is close to user's total, remove user's total
        // to: a) do not leave less than ONE_MILLI_NEAR in the account, b) Allow some yoctos of rounding, e.g. remove(100) removes 99.999993 without panicking
        if is_close(amount_requested, self.available) { // allow for rounding simplification
            self.available
        }
        else {
            amount_requested
        };

        assert!(
            self.available >= to_withdraw,
            "Not enough available balance {} for the requested amount",
            self.available
        );
        self.available -= to_withdraw;

        assert!(main.total_available >= to_withdraw, "i_s_Inconsistency");
        main.total_available -= to_withdraw;

        return to_withdraw;
    }
}
