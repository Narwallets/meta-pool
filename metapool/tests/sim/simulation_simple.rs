#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use near_sdk::{
  borsh::{self, BorshDeserialize, BorshSerialize},
  json_types::{Base58PublicKey, U128},
  serde::{Deserialize, Serialize},
  serde_json::json,
  serde_json::Value,
  *,
};
use near_sdk_sim::{
  account::AccessKey,
  call, deploy, init_simulator,
  near_crypto::{KeyType, SecretKey, Signer},
  to_yocto, view, ContractAccount, ExecutionResult, UserAccount, ViewResult, DEFAULT_GAS,
  STORAGE_AMOUNT,
};

use crate::sim_setup::*;
use crate::sim_utils::*;
use metapool::*;

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
fn simtest_simple() {
  let sim = Simulation::new();

  let metapool = &sim.metapool;

  let view_results = view!(metapool.get_contract_info());
  print_vec_u8("contract_info", &view_results.unwrap());

  sim.show_sps_staked_balances();

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

  //---- alice
  //---- deposit & buy stnear
  let alice = sim.testnet.create_user("alice".to_string(), ntoy(500_000));
  let alice_dep_and_stake = ntoy(100_000);
  {
    let res = call!(
      alice,
      metapool.deposit_and_stake(),
      alice_dep_and_stake,
      50 * TGAS
    );
    check_exec_result(&res);
  }
  assert!(balance(&metapool.user_account) >= alice_dep_and_stake);

  //---- bob
  let bob = sim.testnet.create_user("bob".to_string(), ntoy(500_000));
  let bob_dep_and_stake = ntoy(200_000);
  let bds_res = call!(
    bob,
    metapool.deposit_and_stake(),
    bob_dep_and_stake,
    50 * TGAS
  );

  //---- carol
  let carol = sim.testnet.create_user("carol".to_string(), ntoy(500_000));
  let carol_deposit = ntoy(250_000);

  //let cd_res = call!(carol,metapool.deposit(), carol_deposit, 50*TGAS);
  println!("----------------------------------");
  println!("------- carol adds liquidity --");
  {
    let res = call!(
      carol,
      metapool.nslp_add_liquidity(),
      carol_deposit,
      50 * TGAS
    );
    check_exec_result(&res)
  }

  //contract state
  let view_results = view!(metapool.get_contract_state());
  print_vec_u8("contract_state", &view_results.unwrap());

  println!("----------------------------------");
  println!("------- small qty add-remove liq --");
  {
    let r1 = call!(bob, metapool.nslp_add_liquidity(), 30 * NEAR, 50 * TGAS);
    check_exec_result(&r1);
    let bob_info_1 = sim.show_account_info(&bob.account_id());
    assert!(as_u128(&bob_info_1["nslp_shares"]) == 30 * NEAR);
    let r2 = call!(
      bob,
      metapool.nslp_remove_liquidity(U128::from(30 * NEAR + 9)),
      gas = 100 * TGAS
    );
    check_exec_result(&r2);
    let bob_info_2 = sim.show_account_info(&bob.account_id());
    assert!(as_u128(&bob_info_2["nslp_shares"]) == 0);
    call!(bob, metapool.nslp_add_liquidity(), 30 * NEAR, 50 * TGAS);
    let r4 = call!(
      bob,
      metapool.nslp_remove_liquidity(U128::from(30 * NEAR + 1 - ONE_MILLI_NEAR)),
      gas = 100 * TGAS
    );
    check_exec_result(&r4);
    let bob_info_4 = sim.show_account_info(&bob.account_id());
    assert!(as_u128(&bob_info_4["nslp_shares"]) == 0);
  }

  sim.show_sps_staked_balances();

  //---- test distribute_staking
  println!("----------------------------------");
  println!("------- test distribute_staking --");
  for n in 0..4 {
    println!("------- call #{} to distribute_staking", n);
    let distribute_result = call!(
      sim.operator,
      metapool.distribute_staking(),
      gas = 125 * TGAS
    );
    //check_exec_result_profile(&distribute_result);
    sim.show_sps_staked_balances();
  }
  //check the staking was distributed according to weight
  let total_staked = alice_dep_and_stake + bob_dep_and_stake;
  for n in 0..sim.sp.len() {
    let expected: u128 = total_staked * sim.weight_basis_points_vec[n] as u128 / 100;
    let staked = sim.sp_staked(n);
    assert!(
      staked >= expected - 1 && staked <= expected + 1,
      "total_for_staking:{}, sp{} balance = {}, wbp:{}, !== expected:{}",
      alice_dep_and_stake,
      n,
      &sim.sp_staked(n),
      sim.weight_basis_points_vec[n],
      expected
    );
  }

  //test unstake
  // let unstake_result = view(&sim.sp[0],"unstake_all","{}",0,50*TGAS);
  // check_exec_result_promise(&unstake_result);
  // sim.show_sps_staked_balances();

  //-----------
  sim.show_account_info(&alice.account_id());

  println!("-------------------------");
  println!("------- alice unstakes --");
  let alice_unstaking = ntoy(6_000);
  {
    let ads_res = call!(
      alice,
      metapool.unstake(alice_unstaking.into()),
      gas = 50 * TGAS
    );
    check_exec_result(&ads_res);

    sim.show_account_info(&alice.account_id());
  }

  //------------------------------
  //---- test distribute_unstaking
  println!("------------------------------------");
  println!("------- test distribute_unstaking --");
  for n in 0..20 {
    println!("------- call #{} to distribute_unstaking", n);
    let distribute_result = call!(
      sim.operator,
      metapool.distribute_unstaking(),
      gas = 125 * TGAS
    );
    check_exec_result(&distribute_result);
    sim.show_sps_staked_balances();
    if &distribute_result.unwrap_json_value() == false {
      break;
    };
  }

  //---------------------------------
  //---- test retrieve unstaked funds
  //---------------------------------
  println!("---------------------------------------------");
  println!("------- test retrieve funds from the pools --");
  for n in 0..30 {
    println!(
      "epoch {}",
      view(&sim.get_epoch_acc, "get_epoch_height", "{}")
    );

    println!(
      "------- call #{} to get_staking_pool_requiring_retrieve()",
      n
    );
    let retrieve_result = view!(metapool.get_staking_pool_requiring_retrieve());
    let inx = retrieve_result.unwrap_json_value().as_i64().unwrap();
    println!("------- result {}", inx);

    if inx >= 0 {
      println!("------- pool #{} requires retrieve", inx);
      println!("------- pool #{} sync unstaked", inx);
      let retrieve_result_sync = call!(
        sim.operator,
        metapool.sync_unstaked_balance(inx as u16),
        gas = 200 * TGAS
      );
      check_exec_result(&retrieve_result_sync);
      println!("------- pool #{} retrieve unstaked", inx);
      let retrieve_result_2 = call!(
        sim.operator,
        metapool.retrieve_funds_from_a_pool(inx as u16),
        gas = 200 * TGAS
      );
      check_exec_result_promise(&retrieve_result_2);
    } else if inx == -3 {
      //no more funds unstaked
      break;
    }

    for epoch in 1..4 {
      //make a dummy txn to advance the epoch
      call(
        &sim.owner,
        &sim.get_epoch_acc,
        "set_i32",
        &format!(r#"{{"num":{}}}"#, inx).to_string(),
        0,
        10 * TGAS,
      );
      println!(
        "epoch {}",
        view(&sim.get_epoch_acc, "get_epoch_height", "{}")
      );
    }
  }

  println!("----------------------------------------");
  println!("------- alice calls withdraw_unstaked --");
  {
    let previous = balance(&alice);
    let ads_res = call!(alice, metapool.withdraw_unstaked(), gas = 50 * TGAS);
    check_exec_result(&ads_res);
    assert_less_than_one_milli_near_diff_balance(
      "withdraw_unstaked",
      balance(&alice),
      previous + alice_unstaking,
    );
  }

  println!("---------------------------");
  println!("------- bob liquid-unstakes");
  {
    sim.show_account_info(&bob.account_id());
    sim.show_account_info(&carol.account_id());
    sim.show_account_info(NSLP_INTERNAL_ACCOUNT);
    let vr1 = view!(metapool.get_contract_state());
    print_vec_u8("contract_state", &vr1.unwrap());
    let vr2 = view!(metapool.get_contract_params());
    print_vec_u8("contract_params", &vr2.unwrap());

    let previous = balance(&bob);
    const TO_SELL: u128 = 20_000 * NEAR;
    const MIN_REQUESTED: u128 = 19_300 * NEAR; //7% discount
    let dbp = view!(metapool.nslp_get_discount_basis_points(TO_SELL.into()));
    print_vec_u8("metapool.nslp_get_discount_basis_points", &dbp.unwrap());

    let lu_res = call!(
      bob,
      metapool.liquid_unstake(U128::from(ntoy(20_000)), U128::from(MIN_REQUESTED)),
      0,
      100 * TGAS
    );
    check_exec_result(&lu_res);
    println!("liquid unstake result {}", &lu_res.unwrap_json_value());

    let bob_info = sim.show_account_info(&bob.account_id());
    let carol_info = sim.show_account_info(&carol.account_id());
    let nslp_info = sim.show_account_info(NSLP_INTERNAL_ACCOUNT);

    assert_eq!(as_u128(&bob_info["meta"]), 250 * NEAR);
    assert_eq!(as_u128(&carol_info["meta"]), 1750 * NEAR);
  }

  println!("-----------------------------------");
  println!("------- carol will remove liquidity");
  {
    const AMOUNT: u128 = 100_000 * NEAR;
    println!("-- pre ");
    let pre_balance = balance(&carol);
    println!("pre balance {}", yton(pre_balance));
    let carol_info_pre = sim.show_account_info(&carol.account_id());
    println!("-- nslp_remove_liquidity");
    let res = call!(
      carol,
      metapool.nslp_remove_liquidity(U128::from(AMOUNT)),
      gas = 100 * TGAS
    );
    check_exec_result(&res);
    //let res_json = serde_json::from_str(std::str::from_utf8(&res.unwrap()).unwrap()).unwrap();
    let res_json = res.unwrap_json_value();
    println!("-- result: {:?}", res_json);
    println!("-- after ");
    let carol_info = sim.show_account_info(&carol.account_id());
    let new_balance = balance(&carol);
    println!("new balance {}", yton(new_balance));
    let stnear = as_u128(&carol_info["stnear"]);
    println!("stnear {}", yton(stnear));
    assert_less_than_one_milli_near_diff_balance(
      "rem.liq",
      new_balance + stnear - pre_balance,
      AMOUNT,
    );
  }
}
