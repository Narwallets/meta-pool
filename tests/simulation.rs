#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use near_sdk::{
  borsh::{self, BorshDeserialize, BorshSerialize},
  json_types::U128,
  serde::{Deserialize, Serialize},
  serde_json::json,
  *,
};
use near_sdk_sim::{
  account::AccessKey, call, deploy, init_simulator, near_crypto::Signer, to_yocto, view,
  ContractAccount, ExecutionResult, UserAccount, DEFAULT_GAS, STORAGE_AMOUNT,
  ViewResult
};

// //Note: the struct xxxxxxContract is created by #[near_bindgen] (near_skd_rs~2.0.4)
use divpool::{DiversifiedPool,DiversifiedPoolContract};

// Load contracts' bytes.
near_sdk_sim::lazy_static! {
  static ref WASM_BYTES: &'static [u8] = include_bytes!("../res/divpool.wasm").as_ref();
}

const TGAS: u64 = 1_000_000_000_000;
const NEAR: u128 = 1_000_000_000_000_000_000_000_000;

/// Deploy the contract(s) and create some divpool accounts. Returns:
/// - The divpool Contract
/// - Root Account
/// - Testnet Account (utility suffix for building other addresses)
/// - A deployer account address
fn init_simulator_and_contract(
  initial_balance: u128,
  deploy_to: &str,
) -> (
  ContractAccount<DiversifiedPoolContract>,
  UserAccount, // root
  UserAccount, // testnet suffix
  UserAccount, // deployer account
  UserAccount,
  UserAccount
) {
  // Root account has address: "root"
  let master_account = init_simulator(None);

  // Other accounts may be created from the root account
  // Note: address naming is fully expressive: we may create any suffix we desire, ie testnet, near, etc.
  // but only those two (.testnet, .near) will be used in practice.
  let testnet = master_account.create_user("testnet".to_string(), to_yocto("1000000"));

  // We need an account to deploy the contracts from. We may create subaccounts of "testnet" as follows:
  let owner = testnet.create_user(deploy_to.to_string(), to_yocto("100000"));

  let treasury = testnet.create_user("treasury".to_string(), to_yocto("100000"));
  let operator = testnet.create_user("operator".to_string(), to_yocto("100000"));

  let divpool_contract = deploy!(
      contract: DiversifiedPoolContract,
      contract_id: "divpool",
      bytes: &WASM_BYTES,
      // User deploying the contract
      signer_account: owner,
      // DiversifiedPool.new(
        //   owner_account_id: AccountId,
        //   treasury_account_id: AccountId,
        //   operator_account_id: AccountId,
      deposit:500*NEAR,
      gas:25*TGAS,
      init_method:new(owner.account_id(), treasury.account_id(), operator.account_id())
      );

  return (divpool_contract, master_account, testnet, owner, treasury, operator)
}

/*
fn init_simulated_staking_pool(
    deploy_to: &str,
) -> (
  ContractAccount<DiversifiedPoolContract>,
  UserAccount, // root
  UserAccount, // testnet suffix
  UserAccount, // deployer account
  UserAccount,
  UserAccount
) {
  // Root account has address: "root"
  let master_account = init_simulator(None);

  // Other accounts may be created from the root account
  // Note: address naming is fully expressive: we may create any suffix we desire, ie testnet, near, etc.
  // but only those two (.testnet, .near) will be used in practice.
  let testnet = master_account.create_user("testnet".to_string(), to_yocto("1000000"));

  // We need an account to deploy the contracts from. We may create subaccounts of "testnet" as follows:
  let owner = testnet.create_user(deploy_to.to_string(), to_yocto("100000"));

  let treasury = testnet.create_user("treasury".to_string(), to_yocto("100000"));
  let operator = testnet.create_user("operator".to_string(), to_yocto("100000"));

  let divpool_contract = deploy!(
      contract: DiversifiedPoolContract,
      contract_id: "divpool",
      bytes: &WASM_BYTES,
      // User deploying the contract
      signer_account: owner,
      // DiversifiedPool.new(
        //   owner_account_id: AccountId,
        //   treasury_account_id: AccountId,
        //   operator_account_id: AccountId,
      deposit:500*NEAR,
      gas:25*TGAS,
      init_method:new(owner.account_id(), treasury.account_id(), operator.account_id())
      );

  return (divpool_contract, master_account, testnet, owner, treasury, operator)
}
*/

/// Helper to log ExecutionResult outcome of a call/view
fn print_helper(res: ExecutionResult) {
  println!("Promise results: {:#?}", res.promise_results());
  //println!("Receipt results: {:#?}", res.get_receipt_results());
  println!("Profiling: {:#?}", res.profile_data());
  //println!("Result: {:#?}", res);
  assert!(res.is_ok());
}

 fn print_vecu8(v:&Vec<u8>){
  println!("result: {}", 
   match std::str::from_utf8(v) {
     Ok(v) => v,
     Err(e) => "[[can't decode result, invalid UFT8 sequence]]"
   })
 }

#[test]
fn simtest() {
  
  let (divpool_contract, master_account, testnet, owner, treasury, operator) = init_simulator_and_contract(to_yocto("1000"), "me");

  let view_results = view!(divpool_contract.get_contract_info());
  print_vecu8(&view_results.unwrap());

  //deploy a staking pool - factry style (sollowing the example in https://github.com/near/near-sdk-rs/tree/master/examples/cross-contract-high-level
  // until NEAR core devs can tell me how to deploy a contract into the simulator without having the contract-proxy
  //let sp1 = divpool_contract.deploy_staking_pool("sp1");
  let res = call!(
    owner,
    divpool_contract.deploy_staking_pool("sp1".into(), "sp1_owner".into()),
    STORAGE_AMOUNT,
    DEFAULT_GAS
  );
  print_helper(res);

  let transaction = master_account.create_transaction(
    //"sp1".to_string());  
    ["sp1",".", &divpool_contract.user_account.account_id(),".","testnet"].concat());
  // Creates a signer which contains a public key.
  let res = transaction.transfer(to_yocto("10"))
                      .submit();

  print_helper(res);

  // let res = call!(
  //   deployer,
  //   divpool_contract.MYMETHOD(),
  //   deposit = STORAGE_AMOUNT // send this amount to a payable function, or exclude this line if send 0
  // );
  // print_helper(res);

}
