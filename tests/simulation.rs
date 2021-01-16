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
use divpool::*;

use staking_pool::*;
// Load contracts' bytes.
near_sdk_sim::lazy_static! {
  static ref WASM_BYTES_DIV_POOL: &'static [u8] = include_bytes!("../res/divpool.wasm").as_ref();
  static ref WASM_BYTES_SP: &'static [u8] = include_bytes!("../res/no_wait_staking_pool.wasm").as_ref();
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
      bytes: &WASM_BYTES_DIV_POOL,
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

fn deploy_simulated_staking_pool(
    master_account: &UserAccount,
    deploy_to_acc_id: &str,
    owner_account_id: &str,
) 
  -> UserAccount 
{
  let sp = master_account.deploy(&WASM_BYTES_SP, deploy_to_acc_id.into(), to_yocto("50000"));
  let user_txn = master_account
    .create_transaction(sp.account_id())
    .function_call(
      "new".into(), 
      format!(r#"{{"owner_id":"{}", "stake_public_key":"Di8H4S8HSwSdwGABTGfKcxf1HaVzWSUKVH1mYQgwHCWb","reward_fee_fraction":{{"numerator":5,"denominator":100}}}}"#,
        owner_account_id
        ).into(),//arguments: Vec<u8>,
      50*TGAS, 0);
  let res = user_txn.submit();
  print_helper(res);
  return sp;
}

/// Helper to log ExecutionResult outcome of a call/view
fn print_helper(res: ExecutionResult) {
  println!("Promise results: {:#?}", res.promise_results());
  //println!("Receipt results: {:#?}", res.get_receipt_results());
  //println!("Result: {:#?}", res);
  assert!(res.is_ok());
}
/// Helper to log ExecutionResult outcome of a call/view
fn print_helper_profile(res: ExecutionResult) {
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

fn ntoy(near:u64) -> u128 { to_yocto(&near.to_string()) }

fn yton(yoctos:u128) -> String { 
  let mut str = yoctos.to_string();
  let dec = str.split_off(str.len()-24);
  return [str,".".into(),dec].concat();
}

#[test]
fn simtest() {
  
  let (divpool_contract, master_account, testnet, owner, treasury, operator) = init_simulator_and_contract(to_yocto("1000"), "me");

  let view_results = view!(divpool_contract.get_contract_info());
  print_vecu8(&view_results.unwrap());

  //deploy a staking pool - factory style (following the example in https://github.com/near/near-sdk-rs/tree/master/examples/cross-contract-high-level
  // until NEAR core devs can tell me how to deploy a contract into the simulator without having the contract-proxy
  // master_account.create_user("sp1".into(), ntoy(1_000_000));
  //let sp1 = master_account.deploy(&WASM_BYTES_SP, "sp1".into(), 100*NEAR);
  //divpool_contract.deploy_staking_pool("sp1");
  // let res = call!(
  //   owner,
  //   divpool_contract.deploy_staking_pool("sp1".into(), "sp1_owner".into()),
  //   STORAGE_AMOUNT,
  //   200*TGAS
  // );
  //print_helper(res);

  let sp1 = deploy_simulated_staking_pool(&master_account,"sp1","sp1_owner");

  let transaction = master_account
    .create_transaction("sp1".to_string());  
    //["sp1",".", &divpool_contract.user_account.account_id()].concat());

  //let res = transaction.transfer(ntoy(1)).submit();
  //print_helper(res);

  let view_tx_res = master_account
     .create_transaction(sp1.account_id())
     .function_call("get_owner_id".into(),"{}".into(),25*TGAS, 0)
     .submit();
  print_helper(view_tx_res);

  println!("test: {:#?}", yton(1*NEAR));
  println!("test: {:#?}", yton(10*NEAR));
  println!("test: {:#?}", yton(123*NEAR));
  println!("test: {:#?}", yton(ntoy(1)));
  println!("test: {:#?}", yton(ntoy(10)));
  println!("test: {:#?}", yton(ntoy(123)));

  println!("sp1 amount: {:#?}", yton(sp1.amount()));
  println!("sp1 amount: {}", sp1.amount());

  println!("treasury amount: {:#?}", yton(treasury.amount()));
  //let view_results = view!(divpool_contract.get_contract_info());
  //print_vecu8(&view_results.unwrap());

  // let res = call!(
  //   deployer,
  //   divpool_contract.MYMETHOD(),
  //   deposit = STORAGE_AMOUNT // send this amount to a payable function, or exclude this line if send 0
  // );
  // print_helper(res);

}
