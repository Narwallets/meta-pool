use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::json_types::{U128};
use near_sdk::{env, near_bindgen};
use near_sdk::{AccountId};
use near_sdk::collections::UnorderedMap;

mod internal;
use internal::*;

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc = near_sdk::wee_alloc::WeeAlloc::INIT;

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
#[derive(BorshDeserialize, BorshSerialize)]
pub struct RewardsRegisterContract {
    /// The account ID of the owner 
    pub owner_account_id: AccountId,
    /// Persistent map from an account ID to the corresponding account.
    pub accounts: UnorderedMap<AccountId, Account>,
    /// The total rewards 
    pub total_rewards: u128,
}

impl Default for RewardsRegisterContract {
    fn default() -> Self {
        env::panic(b"This contract should be initialized before usage")  
    }
}

#[near_bindgen]
impl RewardsRegisterContract {

    #[init]
    pub fn new(owner_account_id:String)-> Self{
        /* Prevent re-initializations */
        assert!(!env::state_exists(), "This contract is already initialized");
        return Self {
             owner_account_id,
             total_rewards: 0,
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
        assert!(account.deposited>=1*NEAR, "send at least ONE NEAR to register you account. You'll get back your NEAR when closing the account");
        self.internal_update_account(&account_id, &account);
   
        log!("@{} registered {} as githun_handle",account_id, github_handle);
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

/**************/
/* Unit tests */
/**************/

/*
#[cfg(test)]
mod tests {
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};

    const ONE_NEAR:u128 = 1_000_000_000_000_000_000_000_000;

    /// Set the contract context
    // pub fn initialize() -> &VMContext {
    //     let context = get_context(String::from("client.testnet"), 10);                    
    //     testing_env!(context); 
    //     return &context;
    // }

    /// Defines the context for the contract
    fn get_context(predecessor_account_id: String, storage_usage: u64) -> VMContext {
        VMContext {
            current_account_id: "contract.testnet".to_string(),
            signer_account_id: "alice.testnet".to_string(),
            signer_account_pk: vec![0, 1, 2],
            predecessor_account_id,
            input: vec![],
            block_index: 0,
            block_timestamp: 0,
            account_balance: 0,
            account_locked_balance: 0,
            storage_usage,
            attached_deposit: 0,
            prepaid_gas: 10u64.pow(18),
            random_seed: vec![0, 1, 2],
            is_view: false,
            output_data_receivers: vec![],
            epoch_height: 19,
        }
    }

    //Test get_id and set_id methods
    // #[test]
    // fn test_id() {
    //     let mut context = get_context(String::from("client.testnet"), 10);                    
    //     testing_env!(context); 
    //     /* Initialize contract */
    //     let mut contract = super::RewardsRegisterContract::new(String::from("developers.near"));
    //     let handle = String::from("narwallets");
    //     context.attached_deposit = ONE_NEAR;
    //     contract.set_github_handle(handle.clone());
    //     assert_eq!(contract.get_github_handle(), handle.clone(), "handle is different from the expected");
    // }
}
*/