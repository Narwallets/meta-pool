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


//-- SHARES COMPUTATIONS

/// returns amount * numerator/denominator
pub fn proportional(amount:u128, numerator:u128, denominator:u128) -> u128{
    return (U256::from(amount) * U256::from(numerator) / U256::from(denominator)).as_u128();
}

/// Returns the number of shares corresponding to the given near amount at current share_price
/// if the amount & the shares are incorporated, price remains the same
//
// price = total_amount / total_shares
// Price is fixed
// (total_amount + amount) / (total_shares + num_shares) = total_amount / total_shares
// (total_amount + amount) * total_shares = total_amount * (total_shares + num_shares)
// amount * total_shares = total_amount * num_shares
// num_shares = amount * total_shares / total_amount
pub fn shares_from_amount(amount: u128, total_amount:u128, total_shares:u128 ) -> u128 
{
    if total_shares==0 { //first person getting shares
        return amount;
    }
    if amount==0||total_amount==0 {
        return 0;
    }
    return proportional(total_shares, amount,total_amount);
}

/// Returns the amount corresponding to the given number of shares at current share_price
// price = total_amount / total_shares
// amount = num_shares * price
// amount = num_shares * total_amount / total_shares
pub fn amount_from_shares(num_shares: u128, total_amount:u128, total_shares:u128 ) -> u128 
{
    if total_shares == 0 || num_shares==0 {
        return 0;
    };
    return proportional(num_shares, total_amount,total_shares);
}


