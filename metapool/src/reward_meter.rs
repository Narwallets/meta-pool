use crate::*;

pub use crate::types::*;
pub use crate::utils::*;

// -----------------
// Reward meter utility
// -----------------
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq)]
pub struct RewardMeter {
    ///added with staking
    ///subtracted on unstaking. WARN: Since unstaking can include rewards, delta_staked *CAN BECOME NEGATIVE*
    pub delta_staked: i128, //i128 changing this requires accounts migration
    pub last_multiplier_pct: u16, // (pct: 100 => x1, 200 => x2)
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
    fn open(&self) -> (bool, u128) {
        if self.delta_staked < 0 {
            (true, (-self.delta_staked) as u128)
        } else {
            (false, self.delta_staked as u128)
        }
    }
    /// compute rewards received (extra after stake/unstake)
    /// multiplied by last_multiplier_pct%
    pub fn compute_rewards(&self, valued_shares: u128, currently_distributed: u128, max_to_distribute:u128) -> u128 {
        let (negative, delta) = self.open();
        if negative || delta >= valued_shares {
            return 0; //withdrew all or no positive difference => no rewards, fast exit
        }
        let rewards_by_difference = valued_shares - delta;
        return damp_multiplier(rewards_by_difference, self.last_multiplier_pct, currently_distributed, max_to_distribute);
    }
    ///register a stake (to be able to compute rewards later)
    pub fn stake(&mut self, value: u128) {
        assert!(value <= std::i128::MAX as u128);
        self.delta_staked += value as i128;
    }
    ///register a unstake (to be able to compute rewards later)
    pub fn unstake(&mut self, value: u128) {
        assert!(value <= std::i128::MAX as u128);
        self.delta_staked -= value as i128;
    }

    #[inline]
    pub fn reset(&mut self, valued_shares: u128) {
        assert!(valued_shares <= std::i128::MAX as u128);
        self.delta_staked = valued_shares as i128; // reset meter to Zero difference
    }
    /// compute realized rewards & reset
    /// compute rewards received (extra after stake/unstake) multiplied by last_multiplier_pct%
    /// adds to self.realized
    /// then reset the meter to zero
    /// and maybe update the multiplier
    pub fn realize(&mut self, valued_shares: u128, new_multiplier_pct: u16, currently_distributed: u128, max_to_distribute:u128) -> u128 {
        // note: changed so people can't wait longer to harvest waiting fo a big-multiplier
        self.last_multiplier_pct = new_multiplier_pct; //always apply new multiplier
        let result = self.compute_rewards(valued_shares, currently_distributed, max_to_distribute);
        self.reset(valued_shares); // reset meter to Zero difference
        //self.last_multiplier_pct = new_multiplier_pct; //maybe changed, start applying new multiplier
        return result;
    }
}
