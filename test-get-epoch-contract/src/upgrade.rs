use near_sdk::{env, log, near_bindgen};
#[cfg(target_arch = "wasm32")]
use near_sdk::env::BLOCKCHAIN_INTERFACE;

#[cfg(target_arch = "wasm32")]
const BLOCKCHAIN_INTERFACE_NOT_SET_ERR: &str = "Blockchain interface not set.";

const GAS_FOR_UPGRADE_CODE_AND_MIGRATE:u64 = 150;

use crate::{TestContract, TestContractContract};

#[near_bindgen]
impl TestContract {


}
