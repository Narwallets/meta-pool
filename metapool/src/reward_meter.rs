use crate::*;

pub use crate::types::*;
pub use crate::utils::*;

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
        if self.delta_staked<0 || valued_shares <= (self.delta_staked as u128) { return 0; }

        assert!( valued_shares < ((i128::MAX - self.delta_staked) as u128), "TB");
        // TO-DO remove the i128, make it with 2 fields, it's hard to read and to audit, prone to errors
        // assert!(
        //     self.delta_staked < 0 || valued_shares >= (self.delta_staked as u128),
        //     "valued_shares:{} .LT. self.delta_staked:{}",valued_shares,self.delta_staked
        // );
        // valued_shares - self.delta_staked => true rewards * last_multiplier_pct
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
    /// compute realized rewards & reset
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
