pub use crate::types::*;
use near_sdk::{env, PromiseResult};

#[macro_export]
macro_rules! event {
    ($($arg:tt)*) => ({
        env::log(format!($($arg)*).as_bytes());
    });
}

#[macro_export]
#[cfg(not(prod))]
macro_rules! debug {
    ($($arg:tt)*) => ({
        env::log(format!($($arg)*).as_bytes());
    });
}
#[macro_export]
#[cfg(prod)]
macro_rules! debug {
    
}

pub fn assert_min_balance(amount: u128) {
    assert!(amount > 0, "Amount should be positive");
    assert!(
        env::account_balance() >= MIN_BALANCE_FOR_STORAGE
            && env::account_balance() - MIN_BALANCE_FOR_STORAGE > amount,
        "The contract account balance can't go lower than MIN_BALANCE"
    );
}

pub fn assert_callback_calling() {
    assert_eq!(env::predecessor_account_id(), env::current_account_id());
}

pub fn assert_one_yocto() {
    assert!(
        env::attached_deposit() == 1,
        "the function requires 1 yocto attachment"
    );
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

pub fn apply_pct(basis_points: u16, amount: u128) -> u128 {
    return (U256::from(basis_points) * U256::from(amount) / U256::from(10_000)).as_u128();
}
pub fn apply_multiplier(amount: u128, percentage: u16) -> u128 {
    return (U256::from(amount) * U256::from(percentage as u64 * 10_u64) / U256::from(100)).as_u128();
}


pub fn damp_multiplier(amount: u128, multiplier: u16, currently_distributed: u128, max_to_distribute:u128) -> u128 {
    
    let half = max_to_distribute / 2;
    
    let dampened: u16=
        if multiplier <= 10 || currently_distributed < max_to_distribute {
            multiplier
        }
        else if currently_distributed > max_to_distribute + half {
            10 // back to 1x
        }
        else {
            std::cmp::max(10, multiplier - proportional(multiplier as u128, currently_distributed - max_to_distribute, half) as u16)
        }
    ;
    return apply_multiplier(amount, dampened);
}


//-- SHARED COMPUTATIONS

/// returns amount * numerator/denominator
pub fn proportional(amount: u128, numerator: u128, denominator: u128) -> u128 {
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
pub fn shares_from_amount(amount: u128, total_amount: u128, total_shares: u128) -> u128 {
    if total_shares == 0 {
        //first person getting shares
        return amount;
    }
    if amount == 0 || total_amount == 0 {
        return 0;
    }
    return proportional(total_shares, amount, total_amount);
}

/// Returns the amount corresponding to the given number of shares at current share_price
// price = total_amount / total_shares
// amount = num_shares * price
// amount = num_shares * total_amount / total_shares
pub fn amount_from_shares(num_shares: u128, total_amount: u128, total_shares: u128) -> u128 {
    if total_shares == 0 || num_shares == 0 {
        return 0;
    };
    return proportional(num_shares, total_amount, total_shares);
}

#[inline]
pub fn between(value: u128, from: u128, to: u128) -> bool {
    value > from && value < to
}

/// is_close returns true if total-0.001N < requested < total+0.001N
/// it is used to avoid leaving "dust" in the accounts and to manage rounding simplification for the users
/// e.g.: The user has 999999952342335499220000001 yN => 99.9999952342335499220000001 N
/// the UI shows 5 decimals rounded, so the UI shows "100 N". If the user chooses to liquid_unstake 100 N
/// the contract should take 100 N as meaning "all my tokens", and it will do because:
/// 99.9999952342335499220000001-0.001 < 100 < 99.9999952342335499220000001+0.001
#[inline]
pub fn is_close(requested: u128, total: u128) -> bool {
    requested >= total.saturating_sub(ONE_MILLI_NEAR) && requested <= total + ONE_MILLI_NEAR
}
