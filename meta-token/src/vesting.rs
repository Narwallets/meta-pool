use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env,
    serde::{Deserialize, Serialize},
};

use crate::util::*;

/// Contains information about vesting schedule.
#[derive(BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
#[cfg_attr(feature = "test", derive(Clone, Debug))]
pub struct VestingRecord {
    /// amount locked in  the vesting schedule.
    /// before transferring, vesting is checked and
    /// if we're before cliff_timestamp, locked_amount = amount
    /// else if we're past the end_timestamp, vesting is removed
    /// else we're past the cliff and before end_timestamp, a linear locked_amount is computed
    pub amount: u128,
    /// The timestamp in nanosecond when vesting starts
    /// The remaining tokens will vest linearly until they are fully vested.
    /// Example: 1 year of employment
    pub cliff_timestamp: TimestampNano,
    /// The timestamp in nanosecond when the vesting ends.
    pub end_timestamp: TimestampNano,
}

#[derive(Deserialize, Serialize)]
pub struct VestingRecordJSON {
    pub amount: U128String,
    pub cliff_timestamp: U64String,
    pub end_timestamp: U64String,
}

impl VestingRecord {
    pub fn new(amount: u128, cliff_timestamp: TimestampNano, end_timestamp: TimestampNano) -> Self {
        assert!(amount > 0, "vesting amount must be > 0");
        assert!(
            cliff_timestamp <= end_timestamp,
            "Cliff timestamp can't be later than vesting end timestamp"
        );
        Self {
            amount,
            cliff_timestamp,
            end_timestamp,
        }
    }

    /// Get the amount of tokens that are locked in this account due to vesting or release schedule.
    pub fn compute_amount_locked(&self) -> u128 {
        let block_timestamp = env::block_timestamp();

        return if block_timestamp < self.cliff_timestamp {
            // Before the cliff, all is locked
            self.amount
        } else if block_timestamp >= self.end_timestamp {
            // After end_timestamp none is locked
            0
        } else {
            // compute time-left cannot overflow since block_timestamp < end_timestamp
            let time_left = self.end_timestamp - block_timestamp;
            // The total time is positive. Checked at the contract initialization.
            let total_time = self.end_timestamp - self.cliff_timestamp;
            // locked amount is linearly reduced until time_left = 0 (end_timestamp)
            fraction_of(self.amount, time_left as u128, total_time as u128)
        };
    }
}
