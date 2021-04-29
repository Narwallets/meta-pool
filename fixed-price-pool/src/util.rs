use uint::construct_uint;

#[macro_export]
macro_rules! event {
    ($($arg:tt)*) => ({
        env::log(format!($($arg)*).as_bytes());
    });
}

construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}

/// returns amount * numerator/denominator
pub fn proportional(amount:u128, numerator:u128, denominator:u128) -> u128{
    return (U256::from(amount) * U256::from(numerator) / U256::from(denominator)).as_u128();
}

