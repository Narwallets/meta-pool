use crate::*;
use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::{assert_one_yocto, env, log, near_bindgen, AccountId, Balance, Promise};

// The storage size in bytes for one account + some room, just in order to compute required account storage-rent in yoctoNEARS 
// 3 [1-letter-prefix]+2colons + 64 (acc id) + 16 bytes of u128 (balance) 
pub const ACCOUNT_STORAGE_BYTES: u128 = 3 + 64 + 16;
/// 1e19 yoctos per byte, 0.00001 NEAR per byte, so 100 bytes => 0.001 NEAR, 100Kib => 1 NEAR
/// kept STORAGE_PRICE_PER_BYTE as constant, so people deposit & can retrieve the same amount of NEAR. We cannot depend on env::storage_byte_cost(), we need a constant.
/// if we use env::storage_byte_cost() instead and the result changes in the future, people will be withdrawing a different amount than they deposited
pub const STORAGE_PRICE_PER_BYTE: Balance = 10_000_000_000_000_000_000;
pub const STORAGE_COST : u128 = ACCOUNT_STORAGE_BYTES * STORAGE_PRICE_PER_BYTE;

// We implement the NEP-145 standard. However user can't make additional deposits.
// User registers an account by attaching `storage_deposit()` of NEAR. Deposits above
// that amount will be refunded.
#[near_bindgen]
impl StorageManagement for MetaToken {
    /// Registers an account and records the deposit.
    /// `registration_only` doesn't affect the implementation for vanilla fungible token.
    #[allow(unused_variables)]
    #[payable]
    fn storage_deposit(
        &mut self,
        account_id: Option<ValidAccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        let amount: Balance = env::attached_deposit();
        let account_id: AccountId = if let Some(a) = account_id {
            a.into()
        } else {
            env::predecessor_account_id()
        };
        // check if it is already registered
        let exists = self.accounts.get(&account_id).is_some();
        if exists {
            log!("The account is already registered, refunding the deposit");
            if amount > 0 {
                Promise::new(env::predecessor_account_id()).transfer(amount);
            }
        } else {
            let cost = STORAGE_COST;
            assert!(
                amount >= cost,
                "attached deposit: {},  required: {}",
                amount,
                cost
            );
            self.accounts.insert(&account_id, &0); // register account
            let refund = amount - cost;
            if refund > 0 {
                Promise::new(env::predecessor_account_id()).transfer(refund);
            }
        }
        return storage_balance();
    }

    // While storage_withdraw normally allows the caller to retrieve `available` balance, the basic
    // Fungible Token implementation sets storage_balance_bounds.min == storage_balance_bounds.max,
    // which means available balance will always be 0. So this implementation:
    // * panics if `amount > 0`
    // * never transfers Ⓝ to caller
    // * returns a `storage_balance` struct if `amount` is 0
    #[payable]
    fn storage_withdraw(&mut self, amount: Option<U128>) -> StorageBalance {
        assert_one_yocto();
        let predecessor_account_id = env::predecessor_account_id();
        if self.accounts.contains_key(&predecessor_account_id) {
            match amount {
                Some(amount) if amount.0 > 0 => {
                    env::panic(
                        "The amount is greater than the available storage balance".as_bytes(),
                    );
                }
                _ => storage_balance(),
            }
        } else {
            env::panic(
                format!("The account {} is not registered", &predecessor_account_id).as_bytes(),
            );
        }
    }

    // Returns `true` iff the account was successfully unregistered.
    // Returns `false` iff account was not registered before.
    #[payable]
    fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();
        let force = force.unwrap_or(false);
        if let Some(balance) = self.accounts.get(&account_id) {
            if balance == 0 || force {
                self.accounts.remove(&account_id);
                if balance != 0 {
                    self.total_supply -= balance;
                    // we add 1 because the function requires 1 yocto payment
                    Promise::new(account_id.clone()).transfer(STORAGE_COST + 1);
                }
                return true;
            } else {
                env::panic(
                    "Can't unregister the account with the positive balance without force"
                        .as_bytes(),
                )
            }
        } else {
            log!("The account {} is not registered", &account_id);
            return false;
        }
    }

    fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        let d = U128::from(STORAGE_COST);
        StorageBalanceBounds {
            min: d,
            max: Some(d),
        }
    }

    fn storage_balance_of(&self, account_id: ValidAccountId) -> Option<StorageBalance> {
        if self.accounts.contains_key(account_id.as_ref()) {
            Some(storage_balance())
        } else {
            None
        }
    }
}

// all accounts have the same cost
fn storage_balance() -> StorageBalance {
    StorageBalance {
        total: U128::from(STORAGE_COST),
        available: 0.into(),
    }
}

