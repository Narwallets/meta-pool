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

//
// desk check aka algorithm test plan
//
// Mechanism:
// ---------
// for each step {
//   retrieve state
//   execute method
//   retrieve state
//   compute diffs
//   assert on diffs (according to method)
//   assert on invariants (general)
// }
//
// https://docs.google.com/spreadsheets/d/1VYynsw2yOGIE_0bFdy4CabnI1fnTXDEEffDVbYZSq6Q/edit?usp=sharing
// 

use crate::sim_setup::*;
use crate::sim_utils::*;
use crate::simulation_desk_state::*;
use metapool::*;

//-----------------
fn bot_end_of_epoch_clearing(sim:&Simulation, start:State) -> StateAndDiff {
  return step_call(sim, &sim.operator, "end_of_epoch_clearing", json!({}), 50*TGAS, NO_DEPOSIT, &start);
}

//-----------------
fn bot_distributes(sim:&Simulation, start:State) -> StateAndDiff {

  let mut more_work:bool=true;

  let mut state = start.clone();

  let metapool = &sim.metapool;

  while more_work {
    println!("--CALL metapool.distribute_staking");
    let res = call!(sim.operator, metapool.distribute_staking(),0,150*TGAS);
    check_exec_result(&res);
    more_work = res.unwrap_json();
    println!("--result {}",more_work);
    let post = build_state(&sim);
    let diff = state_diff(&state,&post);
    if more_work|| diff.has_data() {
      println!("--DIFF {}",serde_json::to_string(&diff).unwrap_or_default());
      println!("--POST {}",serde_json::to_string(&post).unwrap_or_default());
    }
    state=post;
    state.assert_invariants();
  }

  more_work=true;
  while more_work {
    println!("--CALL metapool.distribute_unstaking");
    let res = call!(sim.operator, metapool.distribute_unstaking(),0,150*TGAS);
    check_exec_result(&res);
    more_work = res.unwrap_json();
    println!("--result {}",more_work);
    let post = build_state(&sim);
    let diff = state_diff(&state,&post);
    if more_work|| diff.has_data() {
      println!("--DIFF {}",serde_json::to_string(&diff).unwrap_or_default());
      println!("--POST {}",serde_json::to_string(&post).unwrap_or_default());
    }
    state=post;
    state.assert_invariants();
  }

  let diff = state_diff(&start, &state);
  return StateAndDiff { state, diff };
}

//-----------------
fn bot_ping_rewards(sim:&Simulation, start:State) -> StateAndDiff {
  // COMPUTE REWARDS
  //if the epoch is recently started -- ping the pools so they compute rewards and do the same in the meta-pool

  let mut state = start.clone();

  for inx in 0..state.sps.len() {
    let pool = &state.sps[inx];
    let staked = as_u128(&pool["staked"]);
    let unstaked = as_u128(&pool["unstaked"]);
    if (staked > 0 || unstaked > 0) && &pool["last_asked_rewards_epoch_height"] != state.epoch {
      //ping on the pool so it calculates rewards
      println!("about to call PING & DISTRIBUTE on {}", pool.to_string());
      let pool_id = pool["account_id"].as_str().unwrap().to_string();
      let ping=sim.operator.call(pool_id, "ping", &[], 200*TGAS, NO_DEPOSIT);
      check_exec_result(&ping);

      //await near.call(pool.account_id, "ping", {}, OPERATOR_ACCOUNT, credentials.private_key, 200);
      //calculates rewards now in the meta for that pool
      //pub fn distribute_rewards(&mut self, sp_inx: u16) -> void 
      println!("meta.DISTR");
      state = step_call(sim, &sim.operator, "distribute_rewards", json!({"sp_inx":inx}), 200*TGAS, NO_DEPOSIT, &state).state;
    }
  }

  let diff = state_diff(&start, &state);
  return StateAndDiff { state, diff };
}

//-----------------
fn bot_retrieve(sim:&Simulation, start:State) -> StateAndDiff {
  let mut state = start.clone();
  // RETRIEVE UNSTAKED FUNDS
  for inx in 0..state.sps.len() {
    let pool = &state.sps[inx];
    let staked = as_u128(&pool["staked"]);
    let unstaked = as_u128(&pool["unstaked"]);
    if unstaked > 0 && &pool["unstaked_requested_epoch_height"] != "0" {
      println!("about to try RETRIEVE UNSTAKED FUNDS on pool {:?}",pool);
      let now = state.epoch;
      let mut when = as_u128(&pool["unstaked_requested_epoch_height"]) as u64 + NUM_EPOCHS_TO_UNLOCK;
      if when > now+30 {when=now}; //bad data or hard-fork
      if when<=now {
        //try RETRIEVE UNSTAKED FUNDS
        state = step_call(sim, &sim.operator, "retrieve_funds_from_a_pool", json!({"inx":inx}), 200*TGAS, NO_DEPOSIT, &state).state;
      }
    }
  }

  let diff = state_diff(&start, &state);
  return StateAndDiff { state, diff };
}


pub struct StateAndDiff {
  pub state: State,
  pub diff: StateDiff,
}

//-----------
fn step_call( sim:&Simulation, acc:&UserAccount, method:&str, args:Value, gas:u64, attached_near: u128, pre:&State) -> StateAndDiff {
  
  println!("step_call {}",method);
  let res = acc.call( sim.metapool.account_id(), method, args.to_string().as_bytes() , gas, attached_near);// call!(pepe, metapool.nslp_add_liquidity(),10_000*NEAR,200*TGAS);
  check_exec_result(&res);
  let post = build_state(&sim);
  let diff = state_diff(&pre,&post);
  println!("--DIFF {}",serde_json::to_string(&diff).unwrap_or_default());
  println!("--POST {}",serde_json::to_string(&post).unwrap_or_default());
  
  post.assert_invariants();

  return StateAndDiff { state:post, diff };
}

//-----------
impl State {

  pub fn assert_invariants(&self) {

    //delta stake must be = delta stake/unstake orders
    if self.total_for_staking>=self.total_actually_staked {
      let delta_stake = self.total_for_staking - self.total_actually_staked;
      assert!(self.epoch_stake_orders >= self.epoch_unstake_orders);
      let delta_orders = self.epoch_stake_orders - self.epoch_unstake_orders;
      assert_eq!(delta_stake,delta_orders);
    }
    else {
      let delta_unstake = self.total_actually_staked - self.total_for_staking;
      assert!(self.epoch_stake_orders < self.epoch_unstake_orders);
      let delta_orders = self.epoch_unstake_orders - self.epoch_stake_orders;
      assert_eq!(delta_unstake,delta_orders);
    }
    
    assert_eq!(self.contract_account_balance, self.total_available + self.reserve_for_withdraw + self.epoch_stake_orders);

  }

  pub fn assert_rest_state(&self) {

    //we've just cleared orders
    assert_eq!(self.epoch_stake_orders,0);
    assert_eq!(self.epoch_unstake_orders,0);
  
    assert_eq!(self.total_for_staking, self.total_actually_staked);
    assert_eq!(self.total_for_staking, self.staked_in_pools);

    assert_eq!(self.total_unstaked_and_waiting, self.unstaked_in_pools);

    assert_eq!(self.unstake_claims, self.reserve_for_withdraw + self.unstaked_in_pools);

  }

}



#[test]
fn simulation_desk_check() {

  let sim = Simulation::new();

  let metapool = &sim.metapool;

  let contract_info = view!(metapool.get_contract_info());
  print_vec_u8("contract_info", &contract_info.unwrap());

  let contract_state = view!(metapool.get_contract_state());
  print_vec_u8("contract_state", &contract_state.unwrap());

  //starting sp balances
  sim.show_sps_staked_balances();

  //---- Users
  let lucio = sim.testnet.create_user("lucio".to_string(), ntoy(500_000));
  let pepe = sim.testnet.create_user("pepe".to_string(), ntoy(500_000));
  let jose = sim.testnet.create_user("jose".to_string(), ntoy(500_000));
  let maria = sim.testnet.create_user("maria".to_string(), ntoy(500_000));

  let pre = build_state(&sim);
  // initial stake
  println!("--PRE {}",serde_json::to_string(&pre).unwrap_or_default());

  // let mut state: State = pre;
  // let mut diff: StateDiff;

  // step: lucio stakes & the bot distributes in the pools pool
  let amount_initial_stake = 50_000*NEAR;
  let mut result = step_call(&sim, &lucio, "deposit_and_stake", json!({}), 50*TGAS,amount_initial_stake , &pre);
  // state = step_call(&sim, &lucio, "deposit_and_stake", json!({}), 50*TGAS, 10100*NEAR,&state);
  // state = step_call(&sim, &lucio, "deposit_and_stake", json!({}), 50*TGAS, 10200*NEAR,&state);
  // state = step_call(&sim, &lucio, "deposit_and_stake", json!({}), 50*TGAS, 10300*NEAR,&state);

  assert_eq!(result.diff.total_for_staking, amount_initial_stake as i128);
  assert_eq!(result.diff.epoch_stake_orders, amount_initial_stake as i128);

  // step: operator d.stake / d.unstake
  result = bot_distributes(&sim, result.state);
  assert_eq!(result.diff.staked_in_pools, amount_initial_stake as i128);
  
  result = bot_ping_rewards(&sim, result.state);

  // step: pepe add liq
  let amount_add_liq = 10_000*NEAR;
  result = step_call(&sim, &pepe, "nslp_add_liquidity", json!({}), 200*TGAS, amount_add_liq, &result.state);

  assert_eq!(result.diff.total_available, amount_add_liq as i128 );
  assert_eq!(result.diff.contract_account_balance, amount_add_liq as i128);

  // step: lucio d.unstakes
  result = step_call(&sim, &lucio, "unstake", json!({"amount":ntoU128(300)}), 100*TGAS, NO_DEPOSIT, &result.state);

  // step: operator d.stake / d.unstake
  result = bot_distributes(&sim, result.state);
  result = bot_ping_rewards(&sim, result.state);
  result = bot_end_of_epoch_clearing(&sim, result.state);
  result.state.assert_rest_state();

  result = step_call(&sim, &jose, "deposit_and_stake", json!({}), 50*TGAS, 420*NEAR, &result.state);

  result = bot_distributes(&sim, result.state);
  result = bot_ping_rewards(&sim, result.state);
  result = bot_retrieve(&sim, result.state);

  result = step_call(&sim, &lucio, "withdraw", json!({"amount":ntoU128(300)}), 50*TGAS, NO_DEPOSIT, &result.state);

  result = step_call(&sim, &lucio, "unstake", json!({"amount":ntoU128(300)}), 50*TGAS, NO_DEPOSIT, &result.state);

  result = step_call(&sim, &maria, "deposit_and_stake", json!({}), 50*TGAS, 500*NEAR, &result.state);

  result = step_call(&sim, &pepe, "deposit_and_stake", json!({}), 50*TGAS, 150*NEAR, &result.state);

  result = step_call(&sim, &maria, "unstake", json!({"amount":ntoU128(30)}), 50*TGAS, NO_DEPOSIT, &result.state);

  result = bot_distributes(&sim, result.state);
  result = bot_ping_rewards(&sim, result.state);
  result = bot_retrieve(&sim, result.state);
  result = bot_end_of_epoch_clearing(&sim, result.state);

  assert_eq!(result.state.epoch_stake_orders,0);
  assert_eq!(result.state.epoch_unstake_orders,0);
}
