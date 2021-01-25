//use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, Promise};
use crate::*;

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => ({
        env::log(format!($($arg)*).as_bytes());
    });
}

pub const NEAR:u128 = 1_000_000_000_000_000_000_000_000;

impl RewardsRegisterContract {
    /********************/
    /* Internal methods */
    /********************/

    //-- ACCOUNTS --
    /// Inner method to get the given account or a new default value account.
    pub(crate) fn internal_get_account(&self, account_id: &AccountId) -> Account {
        self.accounts.get(account_id).unwrap_or_default()
    }

    /// Inner method to save the given account for a given account ID.
    /// If the account balances are 0, the account is deleted instead to release storage.
    pub(crate) fn internal_update_account(&mut self, account_id: &AccountId, account: &Account) {
        if account.deposited > 0 {
            self.accounts.insert(account_id, &account);
        } else {
            self.accounts.remove(account_id);
        }
    }

    pub(crate) fn internal_close_account(&mut self) {

        let account_id = env::predecessor_account_id();
        let account = self.internal_get_account(&account_id);
        assert!(account.deposited > 0, "No deposit to retreieve");

        self.total_rewards -= account.rewards;

        self.accounts.remove(&account_id);

        log!("@{} closing account. {} returned",&account_id, account.deposited);
        Promise::new(account_id).transfer(account.deposited);

    }

    /// Asserts that the method was called by the owner.
    pub(crate) fn assert_owner(&self) {
        assert!(env::predecessor_account_id()==self.owner_account_id,"Can only be called by the owner")
    }

}
