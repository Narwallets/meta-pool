use crate::*;

use near_sdk::{AccountId};

// /// Amount of gas used for upgrade function itself.
// pub const GAS_FOR_UPGRADE_CALL: Gas = 50_000_000_000_000;
// /// Amount of gas for deploy action.
// pub const GAS_FOR_DEPLOY_CALL: Gas = 20_000_000_000_000;

/* KEEP OLD STATE struct to be able to read from storage
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
*/

#[near_bindgen]
impl TestContract {
    pub fn set_owner(&mut self, owner_id: AccountId) {
        self.assert_owner();
        self.owner_id = owner_id.into();
    }

    // /// Upgrades given contract. Only can be called by owner/DAO.
    // /// if `migrate` is true, calls `migrate()` function right after deployment.
    // /// TODO: consider adding extra grace period in case `owner` got attacked.
    // pub fn upgrade(
    //     &self,
    //     #[serializer(borsh)] code: Vec<u8>
    // ) -> Promise {
    //     self.assert_owner();
    //     let mut promise = Promise::new(env::current_account_id()).deploy_contract(code);
    //     promise = promise.function_call(
    //         "migrate".into(),
    //         vec![],
    //         0,
    //         env::prepaid_gas() - GAS_FOR_UPGRADE_CALL - GAS_FOR_DEPLOY_CALL,
    //     );
    //     promise
    // }

    //-----------------
    //-- migration called after code upgrade
    ///  For next version upgrades, change this function.
    //-- executed after upgrade to NEW CODE
    //-----------------
    /// Should only be called by this contract on upgrade (started from DAO)
    /// Originally a NOOP implementation. KEEP IT if you haven't changed contract state.
    /// If you have changed state, you need to implement migration from old state (keep the old struct with different name to deserialize it first).
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        assert_eq!(
            env::predecessor_account_id(),
            env::current_account_id(),
            "ERR_INVALID_PREDECESSOR"
        );
        //read old state (old structure with different name)
        let old: TestContract/*Old*/ = env::state_read().expect("ERR_CONTRACT_IS_NOT_INITIALIZED");
        //Create the new contract using the data from the old contract.
        let new = TestContract {
            saved_message: old.saved_message,
            saved_i32: old.saved_i32,
            last_epoch: old.last_epoch,
            owner_id: "dao.pool.testnet".into(),
        };
        return new; //return new struct, will be stored as contract state
    }

    pub(crate) fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "ERR_NOT_ALLOWED"
        );
    }
}
