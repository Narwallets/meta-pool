use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{env, log, near_bindgen, ext_contract, is_promise_success, PanicOnDefault, Promise, Gas, Balance};
use near_sdk::json_types::{U128};

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc = near_sdk::wee_alloc::WeeAlloc::INIT;

// const ONE_NEAR:u128 = 1_000_000_000_000_000_000_000_000;
// const ONE_NEAR_CENT:u128 = ONE_NEAR/100;
// const DEPOSIT_FOR_REQUEST: u128 = ONE_NEAR_CENT; // amount that clients have to attach to make a request to the api
// const GAS_FOR_REQUEST: Gas = 50_000_000_000_000;

//contract state
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct TestContract {
    //current request id
    pub saved_message: String,
    pub saved_i32: i32,
    //last response received
    pub last_epoch: u64
}

const ONE_NEAR: Balance  = 1_000_000_000_000_000_000_000_000;
const NEAR: Balance = ONE_NEAR;
const TGAS: Gas = 1_000_000_000_000;
const NO_DEPOSIT:u128 = 0;

type U128String = U128;

#[ext_contract(ext_staking_pool)]
pub trait ExtStakingPool {
    fn get_account_staked_balance(&self, account_id: AccountId) -> U128String;

    fn get_account_unstaked_balance(&self, account_id: AccountId) -> U128String;

    fn get_account_total_balance(&self, account_id: AccountId) -> U128String;

    fn deposit(&mut self);

    fn deposit_and_stake(&mut self);

    fn withdraw(&mut self, amount: U128String);
    fn withdraw_all(&mut self);

    fn stake(&mut self, amount: U128String);

    fn unstake(&mut self, amount: U128String);

    fn unstake_all(&mut self);
}

#[ext_contract(ext_self_owner)]
pub trait ExtMetaStakingPoolOwnerCallbacks {
    fn on_staking_pool_deposit(&mut self, amount: U128String) -> bool;

    fn on_retrieve_from_staking_pool(&mut self, inx: u16) -> bool;

    fn on_staking_pool_stake_maybe_deposit(
        &mut self,
        sp_inx: usize,
        amount: u128,
        included_deposit: bool,
    ) -> bool;

    fn on_staking_pool_unstake(&mut self, sp_inx: usize, amount: u128) -> bool;

    fn on_get_result_from_transfer_poll(&mut self, #[callback] poll_result: PollResult) -> bool;

    fn on_get_sp_total_balance(&mut self, big_amount: u128, #[callback] total_balance: U128String);
    
    fn on_get_sp_unstaked_balance(&mut self, sp_inx: usize, #[callback] unstaked_balance: U128String);

    fn after_minting_meta(&self, account_id:AccountId);

}

#[near_bindgen]
impl TestContract {

    #[init]
    pub fn new()-> Self{
        /* Prevent re-initializations */
        assert!(!env::state_exists(), "This contract is already initialized");
        return Self {
             saved_message: String::from("init"),
             saved_i32: 0,
             last_epoch: env::epoch_height()
         };
    }


    /****************/
    /* Main methods */
    /****************/
    #[payable]
    pub fn set_message(&mut self, message: String){
        self.saved_message = message;
    }
    #[payable]
    pub fn set_i32(&mut self, num: i32){
        self.saved_i32 = num;
    }

    pub fn get_message(&self)-> String{
        return self.saved_message.clone();
    }

    ///Make a request to the dia-gateway smart contract
    pub fn get_epoch_height(&self)-> u64 {
        return env::epoch_height()
    }

    ///Make a request to the dia-gateway smart contract
    pub fn get_block_index(&self)-> u64 {
        return env::block_index()
    }

    ///Test u128 as callback param
    pub fn test_callbacks(&self)-> Promise {

        let big_amount:u128 = u128::MAX; 
        //query our current balance (includes staked+unstaked+staking rewards)
        ext_staking_pool::get_account_total_balance(
            String::from("lucio.testnet"),
            //promise params
            &String::from("meta.pool.testnet"),
            NO_DEPOSIT,
            10*TGAS,
        )
        .then(ext_self_owner::on_get_sp_total_balance(
            big_amount,
            //promise params
            &env::current_account_id(),
            NO_DEPOSIT,
            10*TGAS,
        ))
    }
    //prev-fn continues here
    pub fn on_get_sp_total_balance(big_amount:u128, #[callback] balance:U128String){
        log!("is_promise_success:{} big_amount:{} big_amount(nears):{} balance:{}",
            is_promise_success(), big_amount, big_amount/NEAR, balance.0);
    }

}

/**************/
/* Unit tests */
/**************/

#[cfg(test)]
mod tests {
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};


    /// Set the contract context
    pub fn initialize() {
        let context = get_context(String::from("client.testnet"), 10);                    
        testing_env!(context); 
    }

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

    ///Test get_id and set_id methods
    #[test]
    fn test_id() {
        initialize();
        /* Initialize contract */
        let mut contract = super::TestContract::new();
        let msg = String::from("test string");
        contract.set_message(msg.clone());
        assert_eq!(contract.get_message(), msg.clone(), "Contract message is different from the expected");
    }
}
