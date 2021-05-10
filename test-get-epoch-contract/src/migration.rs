use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{env, near_bindgen};

use crate::*;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct TestContractOld {
    //test state
    pub saved_message: String,
    pub saved_i32: i32,
    //last response received
    pub last_epoch: u64,
    // dao
    //pub controlling_dao:String,
}

#[near_bindgen]
impl TestContract {
    //-----------------
    //-- migration called after code upgrade
    //-- executed after upgrade to NEW CODE
    //-----------------
    /// Should only be called by this contract on upgrade (started from dao)
    /// Originally a NOOP implementation. KEEP IT if you haven't changed contract state.
    /// If you have changed state, you need to implement migration from old state (keep the old struct with different name to deserialize it first).
    /// After migrate goes live on MainNet, return this implementation for next updates.
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        assert_eq!(
            env::predecessor_account_id(),
            env::current_account_id(),
            "ERR_INVALID_PREDECESSOR"
        );
        //read old state (old structure with different name)
        let old: TestContractOld = env::state_read().expect("ERR_CONTRACT_IS_NOT_INITIALIZED");
        //Create the new contract using the data from the old contract.
        let new = TestContract {
            saved_message: old.saved_message,
            saved_i32: old.saved_i32,
            last_epoch: old.last_epoch,
            controlling_dao: "dao.pool.testnet".into(),
        };
        return new; //return new struct, will be stored as contract state
    }
}
