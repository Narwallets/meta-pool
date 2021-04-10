use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::json_types::{U128};
use near_sdk::{env, near_bindgen, setup_alloc, PanicOnDefault, log};
use near_sdk::{AccountId};
use near_sdk::collections::UnorderedMap;

mod internal;
use internal::*;

setup_alloc!();

// const ONE_NEAR_CENT:u128 = ONE_NEAR/100;
// const DEPOSIT_FOR_REQUEST: u128 = ONE_NEAR_CENT; // amount that clients have to attach to make a request to the api
// const GAS_FOR_REQUEST: Gas = 50_000_000_000_000;

/// account data
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq)]
pub struct Account {
    pub github_handle: String,
    pub deposited: u128, //NEAR deposited when registering the account
    pub rewards: u128,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            github_handle: String::from(""),
            deposited: 0,
            rewards: 0,
        }
    }
}

/// Represents an account structure readable by humans.
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct HumanReadableAccount {
    pub account_id: AccountId,
    pub github_handle: String,
    pub rewards: U128,
    pub deposited: U128,
}

//contract state
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct RewardsRegisterContract {
    /// The account ID of the owner 
    pub owner_id: AccountId,
    /// The total rewards 
    pub total_rewards: u128,
    pub total_balance: u128,
    /// Persistent map from an account ID to the corresponding account.
    pub accounts: UnorderedMap<AccountId, Account>,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct RewardsRegisterContractNewVersion {
    /// The account ID of the owner 
    pub owner_id: AccountId,
    /// The total rewards 
    pub total_rewards: u128,
    pub total_balance: u128,
    /// Persistent map from an account ID to the corresponding account.
    pub accounts: UnorderedMap<AccountId, Account>,
}

#[near_bindgen(receiver=None)]
impl RewardsRegisterContract {
    pub fn migrate() {
        let old_state: RewardsRegisterContract = near_sdk::env::state_read().unwrap_or_default();
        let new_state = RewardsRegisterContractNewVersion {
            owner_id : old_state.owner_id,
            total_rewards: old_state.total_rewards,
            total_balance: old_state.total_balance,
            accounts: old_state.accounts
        };
        near_sdk::env::state_write(&new_state);
    }
}

#[near_bindgen]
impl RewardsRegisterContract {

 
    #[init]
    pub fn new(owner_id:String)-> Self{
        /* Prevent re-initializations */
        assert!(!env::state_exists(), "This contract is already initialized");
        return Self {
             owner_id,
             total_rewards: 0,
             total_balance: 0,
             accounts: UnorderedMap::new(b"A".to_vec()),
         };
    }

    /****************/
    /* Main methods */
    /****************/
    #[payable]
    pub fn set_github_handle(&mut self, github_handle: String){
        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);
        let amount = env::attached_deposit();
        account.deposited += amount;
        self.total_balance  += amount;
        assert!(account.deposited>=1*NEAR, "send at least ONE NEAR to register you account. You'll get back your NEAR when closing the account");
        self.internal_update_account(&account_id, &account);
   
        log!("@{} registered {} as github_handle",account_id, github_handle);
    }

    pub fn get_github_handle(&self)-> String {
        let account = self.internal_get_account(&env::predecessor_account_id());
        return account.github_handle.clone();
    }

    pub fn get_rewards(&self)-> U128 {
        let account = self.internal_get_account(&env::predecessor_account_id());
        return account.rewards.into();
    }

    pub fn add_rewards(&mut self,account_id:AccountId,amount:U128) {
        self.assert_owner();
        let mut account = self.internal_get_account(&account_id);
        account.rewards += amount.0;
        self.internal_update_account(&account_id, &account);
    }
    pub fn set_rewards(&mut self,account_id:AccountId,amount:U128) {
        self.assert_owner();
        let mut account = self.internal_get_account(&account_id);
        account.rewards = amount.0;
        self.internal_update_account(&account_id, &account);
    }
    
    pub fn close_account(&mut self){
        self.internal_close_account();
    }

    pub fn get_owner_id(self)-> AccountId { self.owner_id }

    /// Returns human readable representation of the account for the given account ID.
    pub fn get_account(&self, account_id: AccountId) -> HumanReadableAccount {
        self.assert_owner();
        let account = self.internal_get_account(&account_id);
        HumanReadableAccount {
            account_id,
            github_handle:account.github_handle.into(),
            rewards: account.rewards.into(),
            deposited: account.deposited.into(),
        }
    }

    /// Returns the number of accounts that have positive balance on this staking pool.
    pub fn get_number_of_accounts(&self) -> u64 {
        self.accounts.len()
    }

    /// Returns the list of accounts
    pub fn get_accounts(&self, from_index: u64, limit: u64) -> Vec<HumanReadableAccount> {
        self.assert_owner();
        let keys = self.accounts.keys_as_vector();

        (from_index..std::cmp::min(from_index + limit, keys.len()))
            .map(|index| self.get_account(keys.get(index).unwrap()))
            .collect()
    }

}
