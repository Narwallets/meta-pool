//-----------------------------
//-----------------------------
//contract main state migration
//-----------------------------

use near_contract_standards::fungible_token::metadata::FungibleTokenMetadata;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LazyOption;
use near_sdk::collections::LookupMap;
use near_sdk::{env, near_bindgen, AccountId, Balance};

//---------------------------------------------------
//  PREVIOUS Main Contract State for state migrations
//---------------------------------------------------
// uncomment when state migration is required on upgrade

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct PrevContractStateStruct {
    metadata: LazyOption<FungibleTokenMetadata>,
    pub accounts: LookupMap<AccountId, Balance>,
    pub owner_id: AccountId,
    pub minters: Vec<AccountId>,
    pub total_supply: Balance,
}

use crate::MetaToken;
use crate::MetaTokenContract;

#[near_bindgen]
impl MetaToken {
    //-----------------
    //-- migration called after code upgrade
    ///  For next version upgrades, change this function.
    //-- executed after upgrade to NEW CODE
    //-----------------
    /// This fn WILL be called by this contract from `pub fn upgrade` (started from DAO)
    /// Originally a **NOOP implementation. KEEP IT if you haven't changed contract state.**
    /// If you have changed state, you need to implement migration from old state (keep the old struct with different name to deserialize it first).
    ///
    #[init(ignore_state)] //do not auto-load state before this function
    pub fn migrate() -> Self {
        // read state with OLD struct
        // uncomment when state migration is required on upgrade
        //let old: migrations::MetaPoolPrevStateStruct = env::state_read().expect("Old state doesn't exist");
        let old: PrevContractStateStruct = env::state_read().expect("Old state doesn't exist");

        // can only be called by this same contract (it's called from fn upgrade())
        assert_eq!(
            &env::predecessor_account_id(),
            &env::current_account_id(),
            "Can only be called by this contract"
        );

        // uncomment when state migration is required on upgrade
        // NOOP mode, returns this struct that gets stored as contract state
        //return old;

        // uncomment when state migration is required on upgrade
        //
        // Create the new contract state using the data from the old contract state.
        // returns this struct that gets stored as contract state
        return Self {
            metadata: old.metadata,
            accounts: old.accounts,
            owner_id: old.owner_id,
            minters: old.minters,
            total_supply: old.total_supply,
            // new fields
            locked_until_nano: 0,
            vested: LookupMap::new(b"v".to_vec()),
            vested_count: 0,
        };
    }
}
