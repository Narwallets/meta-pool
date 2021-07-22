use near_sdk::json_types::{U128, U64};
use uint::construct_uint;

pub type U128String = U128;
pub type U64String = U64;

/// Raw type for timestamp in nanoseconds
pub type TimestampNano = u64;

construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}

/// returns amount * numerator/denominator
pub fn fraction_of(amount: u128, numerator: u128, denominator: u128) -> u128 {
    return (U256::from(amount) * U256::from(numerator) / U256::from(denominator)).as_u128();
}
