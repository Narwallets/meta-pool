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
  pub delta: u128,
  pub negative: bool,
  /// (pct: 100 => x1, 200 => x2)
  pub last_multiplier_pct: u16,
}

impl Default for RewardMeter {
  fn default() -> Self {
    Self {
      delta: 0,
      negative: false,
      last_multiplier_pct: 100,
    }
  }
}

impl RewardMeter {
  /// compute rewards received (extra after stake/unstake)
  /// multiplied by last_multiplier_pct%
  pub fn compute_rewards(&self, valued_shares: u128) -> u128 {
    if self.negative || self.delta >= valued_shares {
      return 0; //withdrew all or no positive difference => no rewards, fast exit
    }
    let rewards_by_difference = valued_shares - self.delta;
    return apply_multiplier(rewards_by_difference, self.last_multiplier_pct);
  }
  ///register a stake (to be able to compute rewards later)
  pub fn stake(&mut self, value: u128) {
    if self.negative {
      if value >= self.delta {
        //crosses 0
        self.delta = value - self.delta;
        self.negative = false;
      } else {
        //negative, value<delta
        self.delta -= value;
      }
    }
    else {
      self.delta += value;
    }
  }
  ///register a unstake (to be able to compute rewards later)
  pub fn unstake(&mut self, value: u128) {
    if self.negative {
      //already negative
      self.delta += value;
    } else {
      //positive
      if value > self.delta {
        //crosses 0
        self.delta = value - self.delta;
        self.negative = true;
      } else {
        //positive, delta>=value
        self.delta -= value;
      }
    }
  }

  #[inline]
  pub fn reset(&mut self, valued_shares: u128) {
    self.delta = valued_shares; // reset meter to Zero
    self.negative = false;
  }
  /// compute realized rewards & reset
  /// compute rewards received (extra after stake/unstake) multiplied by last_multiplier_pct%
  /// adds to self.realized
  /// then reset the meter to zero
  /// and maybe update the multiplier
  pub fn realize(&mut self, valued_shares: u128, new_multiplier_pct: u16) -> u128 {
    let result = self.compute_rewards(valued_shares);
    self.reset(valued_shares); // reset meter to Zero difference
    self.last_multiplier_pct = new_multiplier_pct; //maybe changed, start applying new multiplier
    return result;
  }

}
