use near_sdk::{env, PromiseResult};
pub use crate::types::*;


pub fn assert_min_balance(amount:u128){
    assert!(amount > 0, "Amount should be positive");
    assert!(
        env::account_balance() >= MIN_BALANCE_FOR_STORAGE && env::account_balance() - MIN_BALANCE_FOR_STORAGE > amount,
        "The contract account balance can't go lower than MIN_BALANCE"
    );
}


pub fn assert_self() {
    assert_eq!(env::predecessor_account_id(), env::current_account_id());
}

pub fn is_promise_success() -> bool {
    assert_eq!(
        env::promise_results_count(),
        1,
        "Contract expected a result on the callback"
    );
    match env::promise_result(0) {
        PromiseResult::Successful(_) => true,
        _ => false,
    }
}

pub fn apply_pct(basis_points:u16, amount:u128) -> u128 {
    return (U256::from(basis_points) * U256::from(amount) / U256::from(10_000)).as_u128() ;
}
