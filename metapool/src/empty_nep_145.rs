use near_contract_standards::storage_management::{StorageBalance, StorageBalanceBounds};
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::{env, near_bindgen};

use crate::*;

// --------------------------------------------------------------------------
// Storage Management (we chose not to require storage backup for this token)
// but ref.finance FE and the WEB wallet seems to be calling theses fns
// --------------------------------------------------------------------------
const EMPTY_STORAGE_BALANCE: StorageBalance = StorageBalance {
    total: U128 { 0: 0 },
    available: U128 { 0: 0 },
};

#[near_bindgen]
impl MetaPool {
    // `registration_only` doesn't affect the implementation for vanilla fungible token.
    #[allow(unused_variables)]
    #[payable]
    pub fn storage_deposit(
        &mut self,
        account_id: Option<ValidAccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        EMPTY_STORAGE_BALANCE
    }

    /// * returns a `storage_balance` struct if `amount` is 0
    pub fn storage_withdraw(&mut self, amount: Option<U128>) -> StorageBalance {
        if let Some(amount) = amount {
            if amount.0 > 0 {
                env::panic(b"The amount is greater than the available storage balance");
            }
        }
        StorageBalance {
            total: 0.into(),
            available: 0.into(),
        }
    }

    #[allow(unused_variables)]
    pub fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        true
    }

    pub fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        StorageBalanceBounds {
            min: U128 { 0: 0 },
            max: Some(U128 { 0: 0 }),
        }
    }

    #[allow(unused_variables)]
    pub fn storage_balance_of(&self, account_id: ValidAccountId) -> Option<StorageBalance> {
        Some(EMPTY_STORAGE_BALANCE)
    }
}
