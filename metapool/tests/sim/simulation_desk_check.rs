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
use crate::sim_steps::*;
use crate::sim_contract_state::*;
use metapool::*;




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
  let mut result = step_call(&sim, &lucio, "deposit_and_stake", json!({}), 50*TGAS,amount_initial_stake , &pre).unwrap();
  // state = step_call(&sim, &lucio, "deposit_and_stake", json!({}), 50*TGAS, 10100*NEAR,&state);
  // state = step_call(&sim, &lucio, "deposit_and_stake", json!({}), 50*TGAS, 10200*NEAR,&state);
  // state = step_call(&sim, &lucio, "deposit_and_stake", json!({}), 50*TGAS, 10300*NEAR,&state);

  assert_eq!(result.diff.total_for_staking, amount_initial_stake as i128);
  assert_eq!(result.diff.epoch_stake_orders, amount_initial_stake as i128);

  // step: operator d.stake / d.unstake
  result = bot_distributes(&sim, &result.state).unwrap();
  assert_eq!(result.diff.staked_in_pools, amount_initial_stake as i128);
  
  result = bot_ping_rewards(&sim, &result.state).unwrap();

  // step: pepe add liq
  let amount_add_liq = 10_000*NEAR;
  result = step_call(&sim, &pepe, "nslp_add_liquidity", json!({}), 200*TGAS, amount_add_liq, &result.state).unwrap();

  assert_eq!(result.diff.total_available, amount_add_liq as i128 );
  assert_eq!(result.diff.contract_account_balance, amount_add_liq as i128);

  // step: lucio d.unstakes
  result = step_call(&sim, &lucio, "unstake", json!({"amount":ntoU128(300)}), 100*TGAS, NO_DEPOSIT, &result.state).unwrap();

  // step: operator d.stake / d.unstake
  result = bot_distributes(&sim, &result.state).unwrap();
  result = bot_ping_rewards(&sim, &result.state).unwrap();
  result = bot_end_of_epoch_clearing(&sim, &result.state).unwrap();
  result.state.assert_rest_state();

  result = step_call(&sim, &jose, "deposit_and_stake", json!({}), 50*TGAS, 420*NEAR, &result.state).unwrap();

  result = bot_distributes(&sim, &result.state).unwrap();
  result = bot_ping_rewards(&sim, &result.state).unwrap();
  result = bot_retrieve(&sim, &result.state).unwrap();

  result = step_call(&sim, &lucio, "withdraw", json!({"amount":ntoU128(300)}), 50*TGAS, NO_DEPOSIT, &result.state).unwrap();

  result = step_call(&sim, &lucio, "unstake", json!({"amount":ntoU128(300)}), 50*TGAS, NO_DEPOSIT, &result.state).unwrap();

  result = step_call(&sim, &maria, "deposit_and_stake", json!({}), 50*TGAS, 500*NEAR, &result.state).unwrap();

  result = step_call(&sim, &pepe, "deposit_and_stake", json!({}), 50*TGAS, 150*NEAR, &result.state).unwrap();

  result = step_call(&sim, &maria, "unstake", json!({"amount":ntoU128(30)}), 50*TGAS, NO_DEPOSIT, &result.state).unwrap();

  result = bot_distributes(&sim, &result.state).unwrap();
  result = bot_ping_rewards(&sim, &result.state).unwrap();
  result = bot_retrieve(&sim, &result.state).unwrap();
  result = bot_end_of_epoch_clearing(&sim, &result.state).unwrap();

  assert_eq!(result.state.epoch_stake_orders,0);
  assert_eq!(result.state.epoch_unstake_orders,0);
}
