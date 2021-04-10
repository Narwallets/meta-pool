#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use near_sdk::{
  borsh::{self, BorshDeserialize, BorshSerialize},
  json_types::{U128, Base58PublicKey},
  serde::{Deserialize, Serialize},
  serde_json::json,
  serde_json::Value,
  *,
};
use near_sdk_sim::{
  account::AccessKey, call, deploy, init_simulator, 
  near_crypto::{Signer,SecretKey, KeyType},
  to_yocto, view,
  ContractAccount, ExecutionResult, UserAccount, DEFAULT_GAS, STORAGE_AMOUNT,
  ViewResult
};

use metapool::*;

// Load contracts' bytes.
near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
  WASM_BYTES_META_POOL => "../res/metapool.wasm",
  //static ref WASM_BYTES_META_POOL: &'static [u8] = include_bytes!("../../res/metapool.wasm").as_ref();
  WASM_BYTES_SP => "../res/staking_pool.wasm",
  // static ref WASM_BYTES_SP: &'static [u8] = include_bytes!("../../res/staking_pool.wasm").as_ref();
  WASM_BYTES_GET_EPOCH => "../res/get_epoch_contract.wasm",
  // static ref WASM_BYTES_GET_EPOCH: &'static [u8] = include_bytes!("../../res/get_epoch_contract.wasm").as_ref();
}

const TGAS: u64 = 1_000_000_000_000;
const NEAR: u128 = 1_000_000_000_000_000_000_000_000;
const ONE_MILLI_NEAR: u128 = NEAR/1_000;
const E24: u128 = NEAR;

const SP_INITIAL_BALANCE:u128 = 35*NEAR;

/// Deploy the contract(s) and create some metapool accounts. Returns:
/// - The metapool Contract
/// - Root Account
/// - Testnet Account (utility suffix for building other addresses)
/// - A deployer account address
//Note: MetaPoolContract is a struct "magically" created by #[near_bindgen] (near_skd_rs~2.0.4)
fn init_simulator_and_contract(
  initial_balance: u128,
  deploy_to: &str,
) -> (
  ContractAccount<MetaPoolContract>,
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
  let testnet = master_account.create_user("testnet".to_string(), ntoy(1_000_000_000));

  // We need an account to deploy the contracts from. We may create sub accounts of "testnet" as follows:
  let owner = testnet.create_user(deploy_to.to_string(), ntoy(1_000_000));

  let treasury = testnet.create_user("treasury".to_string(), ntoy(1_000_000));
  let operator = testnet.create_user("operator".to_string(), ntoy(1_000_000));

  //-- NO MACROS
  // let metapool_contract = master_account.deploy(&WASM_BYTES_META_POOL, account_id: "metapool", STORAGE_AMOUNT);
  
  // metapool_contract.call(
  //   "metapool",
  //   "new",
  //   &json!({
  //       "owner_account_id": owner.account_id(), 
  //       "treasury_account_id": treasury.account_id(), 
  //       "operator_account_id": operator.account_id(),
  //   })
  //   .to_string()
  //   .into_bytes(),
  //   DEFAULT_GAS / 2,
  //   0, // attached deposit
  // )
  // .assert_success();
  //-- END NO MACROS
  
  let metapool_contract = deploy!(
      contract: MetaPoolContract,
      contract_id: "metapool",
      bytes: &WASM_BYTES_META_POOL,
      // User deploying the contract
      signer_account: owner,
      // MetaPool.new(
        //   owner_account_id: AccountId,
        //   treasury_account_id: AccountId,
        //   operator_account_id: AccountId,
      deposit:500*NEAR,
      gas:25*TGAS,
      init_method:new(owner.account_id(), treasury.account_id(), operator.account_id())
      );

  return (metapool_contract, master_account, testnet, owner, treasury, operator)
}

//----------------------
fn view(contract_account: &UserAccount, method:&str, args_json:&str) -> Value {
    // let pct = PendingContractTx {
    //   receiver_id: contract_account.account_id(),
    //   method: method.into(),
    //   args: args_json.into(),
    //   is_view:true,
    // };
    let vr = &contract_account.view(contract_account.account_id(), method, args_json.as_bytes());
    //println!("view Result: {:#?}", vr.unwrap_json_value());
    return vr.unwrap_json_value();
}
fn as_u128(v:&Value) -> u128 {
  return match v.as_str() {
    Some(x) => {
      //println!("{}",x); 
      x.parse::<u128>().unwrap()
    },
    _ => panic!("invalid u128 value {:#?}", v)
  };
}
fn view_u128 (contract_account: &UserAccount, method:&str, args_json:&str) -> u128 {
  let result = view(contract_account,method,args_json);
  return as_u128(&result)
}

//----------------------
fn call(who: &UserAccount, contract_account: &UserAccount, method:&str, args_json:&str, attached_deposit:u128, gas:u64) -> ExecutionResult {
  // let pct = PendingContractTx {
  //   receiver_id: contract_account.account_id(),
  //   method: method.into(),
  //   args: args_json.into(),
  //   is_view:false,
  // };
  let exec_res = who.call(contract_account.account_id(), method, args_json.as_bytes(), gas, attached_deposit);
  //println!("Result: {:#?}", exec_res);
  return exec_res;
}

//-----------------------
fn deploy_simulated_staking_pool(
    master_account: &UserAccount,
    deploy_to_acc_id: &str,
    owner_account_id: &str,
) 
  -> UserAccount 
{
  let sp = master_account.deploy(&WASM_BYTES_SP, deploy_to_acc_id.into(), SP_INITIAL_BALANCE);
  let user_txn = master_account
    .create_transaction(sp.account_id())
    .function_call(
      "new".into(), 
      format!(r#"{{"owner_id":"{}", "stake_public_key":"Di8H4S8HSwSdwGABTGfKcxf1HaVzWSUKVH1mYQgwHCWb","reward_fee_fraction":{{"numerator":5,"denominator":100}}}}"#,
        owner_account_id
        ).into(),//arguments: Vec<u8>,
      50*TGAS, 0);
  let res = user_txn.submit();
  //check_exec_result(res);
  return sp;
}

/// Helper to log ExecutionResult outcome of a call/view
fn check_exec_result(res: &ExecutionResult) {
  //println!("Result: {:#?}", res);
  for line in &res.outcome().logs {
    println!("{:?}",line);
  }
  if !res.is_ok() {
    println!("{:?}",res);
  }
  assert!(res.is_ok());
}
fn check_exec_result_promise(res: &ExecutionResult) {
  //println!("Result: {:#?}", res);
  check_exec_result(res);
  //println!("Receipt results: {:#?}", res.get_receipt_results());
  //println!("Promise results: {:#?}", res.promise_results());
  println!("----Promise results:", );
  let mut inx=0;
  for pr in &res.promise_results() {
    if let Some(some_pr) = pr {
      println!("--promise #{}",inx);
      check_exec_result(&some_pr);
      inx+=1;
    }
  }
  assert!(res.is_ok());
}
/// Helper to log ExecutionResult outcome of a call/view
// fn check_exec_result_profile(res: &ExecutionResult) {
//   println!("Promise results: {:#?}", res.promise_results());
//   //println!("Receipt results: {:#?}", res.get_receipt_results());
//   //println!("Profiling: {:#?}", res.profile_data());
//   //println!("Result: {:#?}", res);
//   assert!(res.is_ok());
// }

 fn print_vec_u8(title:&str, v:&Vec<u8>){
  println!("{}:{}", title,
   match std::str::from_utf8(v) {
     Ok(v) => v,
     Err(e) => "[[can't decode result, invalid UFT8 sequence]]"
   })
 }

fn ntoy(near:u64) -> u128 { to_yocto(&near.to_string()) }

fn yton(yoctos:u128) -> String { 
  let mut str = format!("{:0>25}",yoctos);
  let dec = str.split_off(str.len()-24);
  return [&str,".",&dec].concat();
}

struct Simulation {
  pub metapool: ContractAccount<MetaPoolContract>,
  pub master_account:UserAccount, // root
  pub testnet:UserAccount, // testnet suffix
  pub owner:UserAccount, // deployer account
  pub treasury:UserAccount,
  pub operator:UserAccount,
  pub sp: Vec<UserAccount> //Staking pools
}


const METAPOOL_CONTRACT_ID:&str = "metapool";

//-----------------------------
//-----------------------------
//-----------------------------
impl Simulation {

  pub fn new() -> Self {

    // Root account has address: "root"
    let master_account = init_simulator(None);
    // Other accounts may be created from the root account
    // Note: address naming is fully expressive: we may create any suffix we desire, ie testnet, near, etc.
    // but only those two (.testnet, .near) will be used in practice.
    let testnet = master_account.create_user("testnet".into(), ntoy(1_000_000_000));
    // We need an account to deploy the contracts from. We may create sub accounts of "testnet" as follows:
    let owner = testnet.create_user("contract-owner".into(), ntoy(1_000_000));
    let treasury = testnet.create_user("treasury".into(), ntoy(1_000_000));
    let operator = testnet.create_user("operator".into(), ntoy(1_000_000));

    // NO MACROS -------------
    //create acc, deploy & init the main contract
    // let metapool_contract = master_account.deploy(&WASM_BYTES_META_POOL, account_id: "metapool", STORAGE_AMOUNT);
  
    // metapool_contract.call(
    //   "metapool",
    //   "new",
    //   &json!({
    //       "owner_account_id": owner.account_id(), 
    //       "treasury_account_id": treasury.account_id(), 
    //       "operator_account_id": operator.account_id(),
    //   })
    //   .to_string()
    //   .into_bytes(),
    //   DEFAULT_GAS / 2,
    //   0, // attached deposit
    // )
    // .assert_success();
    // END NO MACROS -------------

    //create acc, deploy & init the main contract
    let metapool = deploy!(
      contract: MetaPoolContract,
      contract_id: &METAPOOL_CONTRACT_ID,
      bytes: &WASM_BYTES_META_POOL,
      // User deploying the contract
      signer_account: &owner,
      // MetaPool.new(
        //   owner_account_id: AccountId,
        //   treasury_account_id: AccountId,
        //   operator_account_id: AccountId,
      deposit:500*NEAR,
      gas:25*TGAS,
      init_method:new(owner.account_id(), treasury.account_id(), operator.account_id())
      );

    //deploy all the staking pools
    let mut sp = Vec::with_capacity(4);
    for n in 0..=3 {
      let sp_contract =deploy_simulated_staking_pool(&master_account, &format!("sp{}",n), &owner.account_id());
      //call(&owner,&sp_contract,"pause_staking","{}",0,10*TGAS);
      sp.push( sp_contract );
    }
    
    return Self {

      metapool,

      master_account,

      testnet,
      owner,
      treasury,
      operator,

      sp,

    }

  }

  pub fn sp_staked(&self, n:usize) -> u128 { 
    view_u128(&self.sp[n],"get_account_staked_balance",&format!(r#"{{"account_id":"{}"}}"#,METAPOOL_CONTRACT_ID))
  }

  // pub fn sp_balance(&self, n:usize) -> u128 { 
  //   if let Some(data) = self.sp[n].account() {
  //     data.amount+data.locked
  //   }
  //   else { 0 }
  //   //self.sp[n].amount()+self.sp[n].locked() 
  // }
  
  pub fn show_sp_balance(&self, n:usize) { 
      //let total = self.sp_balance(n);
      //let data = self.sp[n].account().unwrap();
      //println!("{}",&format!(r#"{{"account_id":"{}"}}"#,&METAPOOL_CONTRACT_ID));
      let staked =  view_u128(&self.sp[n],"get_account_staked_balance",&format!(r#"{{"account_id":"{}"}}"#,METAPOOL_CONTRACT_ID));
      //println!("sp{} get_account_staked_balance:{}, data.amount:{}+data.locked:{}", n, yton(staked));//, data.amount, data.locked ); 
      println!("sp{} get_account_staked_balance:{}", n, yton(staked));//, data.amount, data.locked ); 
  }

  pub fn show_sps_balance(&self){
    println!("--SPs balance");
    for n in 0..=3 { self.show_sp_balance(n) }
    println!("--------------");
  }

  //----------------
  fn show_account_info(&self, acc:&str) -> Value {
    let metapool = &self.metapool;
    let result = view!(metapool.get_account_info(acc.into()));
    print_vec_u8(acc,&result.unwrap());
    //println!("Result: {:#?}", result.unwrap_json_value());
    return serde_json::from_str(std::str::from_utf8(&result.unwrap()).unwrap()).unwrap();
  }

}

pub fn show_balance(ua:&UserAccount) { println!("@{} balance: {}", ua.account_id(), balance(ua) ); }

// #[test]
// fn sim_bug() {
//     let master_account = init_simulator(None);
//     let testnet = master_account.create_user("testnet".into(), ntoy(1_000_000_000));

//     let test_staker = testnet.create_user("staker".to_string(), ntoy(500_000));
//     show_balance(&test_staker);
    
//     let get_epoch_acc = master_account.deploy(&WASM_BYTES_GET_EPOCH, String::from("get_epoch_acc"), SP_INITIAL_BALANCE);
//     let user_txn = master_account
//       .create_transaction(get_epoch_acc.account_id())
//         .function_call(
//           "new".into(), 
//           "{}".into(),
//           50*TGAS, 0)
//         .submit();

//     println!("epoch {}",view(&get_epoch_acc,"get_epoch_height","{}"));

//     let sk = SecretKey::from_seed(KeyType::ED25519, "test");
  
//     //stake => 10K
//     test_staker
//       .create_transaction(test_staker.account_id())
//       .stake(ntoy(10_000),  sk.public_key())
//       .submit();
  
//     show_balance(&test_staker);
//     assert!(test_staker.locked() == ntoy(10_000));
  
//     //stake => 15K
//     test_staker
//       .create_transaction(test_staker.account_id())
//       .stake(ntoy(15_000),  sk.public_key())
//       .submit();
  
//       show_balance(&test_staker);
//       assert!(test_staker.locked() == ntoy(15_000));
    
//     println!("epoch {}",view(&get_epoch_acc,"get_epoch_height","{}"));
      
//     //stake => down to 7K
//     test_staker
//       .create_transaction(test_staker.account_id())
//       .stake(ntoy(7_000),  sk.public_key())
//       .submit();
  
//     show_balance(&test_staker);

//     println!("epoch {}",view(&get_epoch_acc,"get_epoch_height","{}"));

//     //do it 15 times
//     //in the sim => 3 blocks make and epoch
//     for n in 0..5 {
//       call(&test_staker,&get_epoch_acc,"set_i32", &format!(r#"{{"num":{}}}"#,n),0,10*TGAS);
//       println!("epoch {}",view(&get_epoch_acc,"get_epoch_height","{}"));
//     }

//     show_balance(&test_staker);

//     //stake => down to 7K
//     test_staker
//       .create_transaction(test_staker.account_id())
//       .stake(ntoy(7_000),  sk.public_key())
//       .submit();

//     assert!(test_staker.locked() == ntoy(7_000));
// }


#[test]
fn simtest() {
  
  let sim = Simulation::new();

  let metapool = &sim.metapool;

  let view_results = view!(metapool.get_contract_info());
  print_vec_u8("contract_info",&view_results.unwrap());

  sim.show_sps_balance();

  //Example transfer to account
  // let transaction = master_account
  //   .create_transaction("sp1".to_string());  
    //["sp1",".", &metapool_contract.user_account.account_id()].concat());
  //let res = transaction.transfer(ntoy(1)).submit();
  //check_exec_result(res);

  //test sp1 exists
  //println!("sp0 owner {}",view_call(&sim.sp[0], "get_owner_id", "{}"));

  // test yton & ntoy
  // println!("test: {}", yton(1*NEAR));
  // println!("test: {}", yton(10*NEAR));
  // println!("test: {}", yton(123*NEAR));
  // println!("test: {}", yton(ntoy(1)));
  // println!("test: {}", yton(ntoy(10)));
  // println!("test: {}", yton(ntoy(123)));

  //println!("treasury amount: {}", sim.treasury.amount());

  //---- register staking pools in the metapool contract
  let weight_basis_points_vec = vec!(15,40,25,20);
  for n in 0..sim.sp.len() {
    call!(sim.owner, metapool.set_staking_pool(sim.sp[n].account_id(),weight_basis_points_vec[n]*100), gas=25*TGAS);
  }
  let total_w_bp = view!(metapool.sum_staking_pool_list_weight_basis_points());
  assert!(total_w_bp.unwrap_json_value() == 10000);

  //---- alice
  //---- deposit & buy stnear
  let alice = sim.testnet.create_user("alice".to_string(), ntoy(500_000));
  let alice_dep_and_stake = ntoy(100_000);
  let ads_res = call!(alice,metapool.deposit_and_stake(), alice_dep_and_stake, 50*TGAS);
  //check_exec_result(&ads_res);
  assert!(balance(&metapool.user_account)>=alice_dep_and_stake);

  //---- bob
  let bob = sim.testnet.create_user("bob".to_string(), ntoy(500_000));
  let bob_dep_and_stake = ntoy(200_000);
  let bds_res = call!(bob,metapool.deposit_and_stake(), bob_dep_and_stake, 50*TGAS);

  //---- carol
  let carol = sim.testnet.create_user("carol".to_string(), ntoy(500_000));
  let carol_deposit = ntoy(250_000);
  //let cd_res = call!(carol,metapool.deposit(), carol_deposit, 50*TGAS);
  println!("----------------------------------");
  println!("------- carol adds liquidity --");
  let cal_res = call!(carol,metapool.nslp_add_liquidity(), carol_deposit, 50*TGAS );

  //contract state
  let view_results = view!(metapool.get_contract_state());
  print_vec_u8("contract_state",&view_results.unwrap());

  {
    println!("----------------------------------");
    println!("------- small qty add-remove liq --");
    let r1=call!(bob,metapool.nslp_add_liquidity(), 30*NEAR, 50*TGAS );
    check_exec_result(&r1);
    let bob_info_1 =sim.show_account_info(&bob.account_id());
    assert!(as_u128(&bob_info_1["nslp_shares"]) == 30*NEAR);
    let r2=call!(bob,metapool.nslp_remove_liquidity(U128::from(30*NEAR+9)), gas=100*TGAS);
    check_exec_result(&r2);
    let bob_info_2 =sim.show_account_info(&bob.account_id());
    assert!(as_u128(&bob_info_2["nslp_shares"]) == 0);
    call!(bob,metapool.nslp_add_liquidity(), 30*NEAR, 50*TGAS );
    let r4=call!(bob,metapool.nslp_remove_liquidity(U128::from(30*NEAR + 1 - ONE_MILLI_NEAR)), gas=100*TGAS);
    check_exec_result(&r4);
    let bob_info_4 =sim.show_account_info(&bob.account_id());
    assert!(as_u128(&bob_info_4["nslp_shares"]) == 0);
  }

  //---- test distribute_staking
  sim.show_sps_balance();
  println!("----------------------------------");
  println!("------- test distribute_staking --");
  for n in 0..4 {
    println!("------- call #{} to distribute_staking",n);
    let distribute_result = call!(sim.operator, metapool.distribute_staking(), gas=125*TGAS );
    //check_exec_result_profile(&distribute_result);
    sim.show_sps_balance();
  }
  
  //check the staking was distributed according to weight
  let total_staked = alice_dep_and_stake + bob_dep_and_stake;
  for n in 0..sim.sp.len() {
    let expected:u128 = total_staked * weight_basis_points_vec[n] as u128 / 100;
    let staked = sim.sp_staked(n);
    assert!( staked >= expected - 1 && staked <= expected + 1,
      "total_for_staking:{}, sp{} balance = {}, wbp:{}, !== expected:{}", alice_dep_and_stake, n, &sim.sp_staked(n), weight_basis_points_vec[n], expected);
  }

  //test unstake
  // let unstake_result = view(&sim.sp[0],"unstake_all","{}",0,50*TGAS);
  // check_exec_result_promise(&unstake_result);
  // sim.show_sps_balance();

  //----------------------------------------------------------
  sim.show_account_info(&alice.account_id());

  //----------------------------------------------------------
  println!("----------------------------------");
  println!("------- alice unstakes --");
  let alice_unstaking = ntoy(6_000);
  let ads_res = call!(alice,metapool.unstake(alice_unstaking.into()), gas=50*TGAS);
  check_exec_result(&ads_res);

  //----------------------------------------------------------
  sim.show_account_info(&alice.account_id());

  //----------------------------------------------------------
  //---- test distribute_unstaking
  println!("----------------------------------");
  println!("------- test distribute_unstaking --");
  for n in 0..20 {
    println!("------- call #{} to distribute_unstaking",n);
    let distribute_result = call!(sim.operator, metapool.distribute_unstaking(), gas=125*TGAS );
    check_exec_result(&distribute_result);
    sim.show_sps_balance();
    if &distribute_result.unwrap_json_value()==false { break };
  }

  //deploy a contract to get the current epoch
  let get_epoch_acc = sim.master_account.deploy(&WASM_BYTES_GET_EPOCH, String::from("get_epoch_acc"), SP_INITIAL_BALANCE);
  let user_txn = sim.master_account
    .create_transaction(get_epoch_acc.account_id())
      .function_call(
        "new".into(), 
        "{}".into(),
        50*TGAS, 0)
      .submit();

  //----------------------------------------------------------
  //---- test retrieve unstaked funds
  println!("----------------------------------");
  println!("------- test retrieve funds from the pools --");
  for n in 0..30 {
    
    println!("epoch {}",view(&get_epoch_acc,"get_epoch_height","{}"));

    println!("------- call #{} to get_staking_pool_requiring_retrieve()",n);
    let retrieve_result = view!(metapool.get_staking_pool_requiring_retrieve());
    let inx = retrieve_result.unwrap_json_value().as_i64().unwrap();
    println!("------- result {}",inx);

    if inx>=0 {
      println!("------- pool #{} requires retrieve",inx);
      println!("------- pool #{} sync unstaked",inx);
      let retrieve_result_sync = call!(sim.operator, metapool.sync_unstaked_balance(inx as u16), gas=200*TGAS );
      check_exec_result(&retrieve_result_sync);
      println!("------- pool #{} retrieve unstaked",inx);
      let retrieve_result_2 = call!(sim.operator, metapool.retrieve_funds_from_a_pool(inx as u16), gas=200*TGAS );
      check_exec_result_promise(&retrieve_result_2);
    }
    else if inx==-3 { //no more funds unstaked
      break;
    }

    for epoch in 1..4 {
      //make a dummy txn to advance the epoch
      call(&sim.owner, &get_epoch_acc,"set_i32",&format!(r#"{{"num":{}}}"#,inx).to_string(),0,10*TGAS);
      println!("epoch {}",view(&get_epoch_acc,"get_epoch_height","{}"));
    }
  }

  //----------------------------------------------------------
  {
    println!("----------------------------------");
    println!("------- alice calls withdraw_unstaked --");
    let previous = balance(&alice);
    let ads_res = call!(alice,metapool.withdraw_unstaked(), gas=50*TGAS);
    check_exec_result(&ads_res);
    assert_less_than_one_milli_near_diff_balance("withdraw_unstaked",balance(&alice),previous+alice_unstaking);
  }


  //----------------------------------------------------------
  {
    println!("----------------------------------");
    println!("------- bob liquid-unstakes");

    sim.show_account_info(&bob.account_id());
    sim.show_account_info(&carol.account_id());
    sim.show_account_info(NSLP_INTERNAL_ACCOUNT);
    let vr1 = view!(metapool.get_contract_state());
    print_vec_u8("contract_state",&vr1.unwrap());
    let vr2 = view!(metapool.get_contract_params());
    print_vec_u8("contract_params",&vr2.unwrap());
    

    let previous = balance(&bob);
    const TO_SELL:u128 = 20_000*NEAR;
    const MIN_REQUESTED:u128 = 19_300*NEAR; //7% discount
    
    let dbp = view!(metapool.nslp_get_discount_basis_points(TO_SELL.into()));
    print_vec_u8("metapool.nslp_get_discount_basis_points",&dbp.unwrap());

    let lu_res = call!(bob,metapool.liquid_unstake(U128::from(ntoy(20_000)),U128::from(MIN_REQUESTED)), 0, 100*TGAS);
    check_exec_result(&lu_res);
    println!("liquid unstake result {}",&lu_res.unwrap_json_value());

    let bob_info = sim.show_account_info(&bob.account_id());
    let carol_info =sim.show_account_info(&carol.account_id());
    let nslp_info = sim.show_account_info(NSLP_INTERNAL_ACCOUNT);

    assert_eq!(as_u128(&bob_info["meta"]), 250*E24);
    assert_eq!(as_u128(&carol_info["meta"]), 1750*E24);
    
  }

  //----------------------------------------------------------
  {
    println!("----------------------------------");
    const AMOUNT:u128 = 100_000*NEAR;
    println!("------- carol will remove liquidity");
    println!("-- pre ");
    let pre_balance = balance(&carol);
    println!("pre balance {}", yton(pre_balance));
    let carol_info_pre = sim.show_account_info(&carol.account_id());
    println!("-- nslp_remove_liquidity");
    let res = call!(carol,metapool.nslp_remove_liquidity(U128::from(AMOUNT)), gas=100*TGAS);
    check_exec_result(&res);
    //let res_json = serde_json::from_str(std::str::from_utf8(&res.unwrap()).unwrap()).unwrap();
    let res_json = res.unwrap_json_value();
    println!("-- result: {:?}",res_json);
    println!("-- after ");
    let carol_info = sim.show_account_info(&carol.account_id());
    let new_balance = balance(&carol);
    println!("new balance {}", yton(new_balance));
    let stnear = as_u128(&carol_info["stnear"]);
    println!("stnear {}", yton(stnear));
    assert_less_than_one_milli_near_diff_balance("rem.liq", new_balance + stnear - pre_balance,  AMOUNT);
  }


}

pub fn assert_less_than_one_milli_near_diff_balance(action:&str, bal:u128, expected:u128) -> bool {
  if bal==expected {return true};
  if bal>expected {
    panic!("{} failed MORE THAN EXPECTED diff:{} bal:{} expected:{}", 
          action,yton(bal-expected), yton(bal), yton(expected));
  }
  let differ = expected-bal;
  if differ<ONE_MILLI_NEAR {return true};
  panic!("{} failed LESS THAN EXPECTED by more than 0.001 diff:{} bal:{} expected:{}", 
        action,yton(differ), yton(bal), yton(expected));
}

pub fn balance(acc:&UserAccount) -> u128 { 
  if let Some(data) = acc.account() {
    data.amount+data.locked
  }
  else { 0 }
  //self.sp[n].amount()+self.sp[n].locked() 
}
