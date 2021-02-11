use crate::*;

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
    /// since statking is delayed and in batches it only eventually matches env::balance()
    /// total = available + staked + unstaked
    pub available: u128,

    /// The amount of shares of the total staked balance in the pool(s) this user owns.
    /// Before someone stakes share-price is computed and shares are "sold" to the user so he only owns what he's staking and no rewards yet
    /// When a user reequest a transfer to other user, staked & shares from the origin are moved to staked & shares of the destination
    /// The share_price can be computed as total_for_staking/total_stake_shares
    /// shares * share_price = stNEARs
    pub stake_shares: u128,

    /// Incremented when the user asks for unstaking. The amount of unstaked near in the pools
    pub unstaked: u128,

    /// The epoch height when the unstaked will be available
    /// The fund will be locked for -AT LEAST- NUM_EPOCHS_TO_UNLOCK epochs
    pub unstaked_requested_unlock_epoch: EpochHeight,

    //-- META
    ///realized META, can be used to transfer META from one user to another
    // Total META = realized_meta + staking_meter.mul_rewards(valued_stake_shares) + lp_meter.mul_rewards(valued_lp_shares)
    // Every time the user operates on STAKE/UNSTAKE: we realize meta: realized_meta += staking_meter.mul_rewards(valued_staked_shares)
    // Every time the user operates on ADD.LIQ/REM.LIQ.: we realize meta: realized_meta += lp_meter.mul_rewards(valued_lp_shares)
    // if the user calls farm_meta() we perform both
    pub realized_meta: u128,
    ///Staking rewards meter (to mint stnear for the user)
    pub staking_meter: RewardMeter,
    ///LP fee gains meter (to mint meta for the user)
    pub lp_meter: RewardMeter,

    //-- STATISTICAL DATA --
    // User's statistical data
    // This is the user-cotrolled staking rewards meter, it works as a car's "trip meter". The user can reset them to zero.
    // to compute trip_rewards we start from current_stnear, undo unstakes, undo stakes and finally subtract trip_start_stnear
    // trip_rewards = current_stnear + trip_accum_unstakes - trip_accum_stakes - trip_start_stnear;
    /// trip_start: (timpestamp in miliseconds) this field is set at account creation, so it will start metering rewards
    pub trip_start: Timestamp,

    /// How much stnears the user had at "trip_start".
    pub trip_start_stnear: u128,
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
            unstaked_requested_unlock_epoch: 0,
            //meta & reward-meters
            realized_meta: 0,
            staking_meter: RewardMeter::default(),
            lp_meter: RewardMeter::default(),
            //trip-meter fields
            trip_start: env::block_timestamp() / 1_000_000, //converted from nanoseconds to miliseconds
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
    pub fn valued_nslp_shares(&self, main: &DiversifiedPool, nslp_account: &Account) -> u128 { main.amount_from_nslp_shares(self.nslp_shares, &nslp_account) }

    /// return realized meta plus pending rewards
    pub fn total_meta(&self, main: &DiversifiedPool) -> u128 {
        let valued_stake_shares = main.amount_from_stake_shares(self.stake_shares);
        let nslp_account = main.internal_get_nslp_account();
        let valued_lp_shares = self.valued_nslp_shares(main, &nslp_account);
        //debug!("self.realized_meta:{}, self.staking_meter.compute_rewards(valued_stake_shares):{} self.lp_meter.compute_rewards(valued_lp_shares):{}",
        //    self.realized_meta, self.staking_meter.compute_rewards(valued_stake_shares), self.lp_meter.compute_rewards(valued_lp_shares));
        return self.realized_meta
            + self.staking_meter.compute_rewards(valued_stake_shares)
            + self.lp_meter.compute_rewards(valued_lp_shares);
    }


    //---------------------------------
    pub fn stake_realize_meta(&mut self, main:&mut DiversifiedPool) {
        //realize meta pending rewards on LP operation
        let valued_actual_shares = main.amount_from_stake_shares(self.stake_shares);
        let pending_meta = self.staking_meter.realize(valued_actual_shares, main.staker_meta_mult_pct);
        self.realized_meta += pending_meta;
        main.total_meta += pending_meta;
    }

    pub fn nslp_realize_meta(&mut self, nslp_account:&Account, main:&mut DiversifiedPool)  {
        //realize meta pending rewards on LP operation
        let valued_actual_shares = self.valued_nslp_shares(main, &nslp_account);
        let pending_meta = self.lp_meter.realize(valued_actual_shares, main.lp_provider_meta_mult_pct);
        self.realized_meta += pending_meta;
        main.total_meta += pending_meta;
    }

    //----------------
    pub fn add_stake_shares(&mut self, num_shares:u128, stnear:u128){
        self.stake_shares += num_shares;
        //to buy stnear is stake
        self.trip_accum_stakes += stnear;
        self.staking_meter.stake(stnear);
    }
    pub fn sub_stake_shares(&mut self, num_shares:u128, stnear:u128){
        assert!(self.stake_shares>=num_shares,"sub_stake_shares self.stake_shares {} < num_shares {}",self.stake_shares,num_shares);
        self.stake_shares -= num_shares;
        //to sell stnear is to unstake
        self.trip_accum_unstakes += stnear;
        self.staking_meter.unstake(stnear);
    }

    /// user method
    /// completes unstake action by moving from retreieved_from_the_pools to available
    pub fn try_finish_unstaking(&mut self, main:&mut DiversifiedPool) {

        let amount = self.unstaked;
        assert!(amount > 0, "No unstaked balance");
        
        let epoch = env::epoch_height();
        assert!( epoch >= self.unstaked_requested_unlock_epoch,
            "The unstaked balance is not yet available due to unstaking delay. You need to wait at least {} epochs"
            , self.unstaked_requested_unlock_epoch - epoch);

        //use retrieved funds
        // moves from total_actually_unstaked_and_retrieved to total_available
        assert!(main.total_actually_unstaked_and_retrieved >= amount, "Funds are not yet available due to unstaking delay. Epoch:{}",env::epoch_height());
        main.total_actually_unstaked_and_retrieved -= amount;
        main.total_available += amount;
        // in the account, moves from unstaked to available
        self.unstaked -= amount; //Zeroes
        self.available += amount;
    }


}
