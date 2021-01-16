use near_sdk::{near_bindgen,Promise};

use crate::*;

//pub use crate::types::*;
//pub use crate::utils::*;

#[near_bindgen]
impl DiversifiedPool {

    pub fn deploy_staking_pool(&self, account_id: String, owner_account_id: String) {

        env::log(format!("{} creating {}",env::current_account_id(),account_id).as_bytes());

        Promise::new(account_id)
            .create_account()
            .transfer(100*ONE_NEAR)
            .add_full_access_key(env::signer_account_pk())
            .deploy_contract(
                include_bytes!("../res/no_wait_staking_pool.wasm").to_vec(),
            )
            .function_call(
                "new".into() , //method_name: Vec<u8>,
                format!(r#"{{"owner_id":"{}", "stake_public_key":"Di8H4S8HSwSdwGABTGfKcxf1HaVzWSUKVH1mYQgwHCWb","reward_fee_fraction":{{"numerator":5,"denominator":100}}}}"#,
                    owner_account_id
                    ).into(),//arguments: Vec<u8>,
                0,//amount: Balance,
                100*gas::TGAS //gas: Gas,
            );
        }
}