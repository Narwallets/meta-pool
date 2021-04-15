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


///
/// https://docs.google.com/spreadsheets/d/1VYynsw2yOGIE_0bFdy4CabnI1fnTXDEEffDVbYZSq6Q/edit?usp=sharing
/// 
#[derive(Debug,Serialize,Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct State {
  
  pub epoch: u64,

  pub contract_account_balance: u128,
  pub reserve_for_withdraw: u128,
  pub total_available:u128,

  pub epoch_stake_orders:u128,
  pub epoch_unstake_orders:u128,

  pub total_for_staking: u128,
  pub total_actually_staked: u128,
  pub to_stake_delta: i128,

  pub total_unstaked_and_waiting: u128,

  pub unstake_claims: u128,
  pub unstake_claims_available_sum: u128, //how much we have to fulfill unstake claims

  pub staked_in_pools: u128,
  pub unstaked_in_pools: u128,
  pub total_in_pools: u128,

  pub sps:Vec<Value>,
}

#[derive(Debug,Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct StateDiff {
  pub contract_account_balance: i128,
  pub reserve_for_withdraw: i128,
  pub total_available:i128,

  pub epoch_stake_orders:i128,
  pub epoch_unstake_orders:i128,

  pub total_for_staking: i128,
  pub total_actually_staked: i128,
  pub to_stake_delta: i128,

  pub total_unstaked_and_waiting: i128,

  pub unstake_claims: i128,
  pub unstake_claims_available_sum: i128, //how much we have to fulfill unstake claims

  pub staked_in_pools: i128,
  pub unstaked_in_pools: i128,
  pub total_in_pools: i128,
}
impl StateDiff {
  pub fn has_data(&self) -> bool {
    self.contract_account_balance != 0
    || self.reserve_for_withdraw != 0
    || self.total_available != 0
  
    || self.epoch_stake_orders != 0
    || self.epoch_unstake_orders != 0
  
    || self.total_for_staking != 0
    || self.total_actually_staked != 0
    || self.to_stake_delta != 0
  
    || self.total_unstaked_and_waiting != 0
  
    || self.unstake_claims != 0
    || self.unstake_claims_available_sum != 0
  
    || self.staked_in_pools != 0
    || self.unstaked_in_pools != 0
    || self.total_in_pools != 0
  
  }
}

pub fn build_state(sim:&Simulation) -> State {
  
  let metapool = &sim.metapool;
  let contract_state = view!(metapool.get_contract_state()).unwrap_json_value();

  let total_for_staking= as_u128(&contract_state["total_for_staking"]);
  let total_actually_staked= as_u128(&contract_state["total_actually_staked"]);

  let reserve_for_withdraw = as_u128(&contract_state["reserve_for_unstake_claims"]);
  let total_unstaked_and_waiting = as_u128(&contract_state["total_unstaked_and_waiting"]);

  let view_result = view!(metapool.get_staking_pool_list());
  let sps:Vec<Value> = near_sdk::serde_json::from_slice(&view_result.unwrap()).unwrap_or_default();

  let mut sum_staked:u128=0;
  let mut sum_unstaked:u128=0;
  for sp in &sps {
    sum_staked+=as_u128(&sp["staked"]);
    sum_unstaked+=as_u128(&sp["unstaked"]);
  }

  let to_stake_delta = total_for_staking as i128 - total_actually_staked as i128;

  return State {
    
    epoch: as_u128(&contract_state["env_epoch_height"]) as u64,

    contract_account_balance: as_u128(&contract_state["contract_account_balance"]),
    reserve_for_withdraw,
    total_available: as_u128(&contract_state["total_available"]),
  
    epoch_stake_orders:as_u128(&contract_state["epoch_stake_orders"]),
    epoch_unstake_orders:as_u128(&contract_state["epoch_unstake_orders"]),
  
    total_for_staking,
    total_actually_staked,
    to_stake_delta,
  
    total_unstaked_and_waiting,
  
    unstake_claims: as_u128(&contract_state["total_unstake_claims"]),
    unstake_claims_available_sum: reserve_for_withdraw + total_unstaked_and_waiting + if to_stake_delta<0 { (-to_stake_delta) as u128 } else {0}, //to_stake_delta neg means unstake to be made
  
    staked_in_pools: sum_staked,
    unstaked_in_pools: sum_unstaked,
    total_in_pools: sum_staked+sum_unstaked,
    
    sps,
  }
}

pub fn state_diff(pre:&State, post:&State) -> StateDiff {
  return StateDiff {
    contract_account_balance: post.contract_account_balance as i128 - pre.contract_account_balance as i128,
    reserve_for_withdraw:  post.reserve_for_withdraw as i128 - pre.reserve_for_withdraw as i128,
    total_available: post.total_available as i128 - pre.total_available as i128,
  
    epoch_stake_orders: post.epoch_stake_orders as i128 - pre.epoch_stake_orders as i128,
    epoch_unstake_orders: post.epoch_unstake_orders as i128 - pre.epoch_unstake_orders as i128,
  
    total_for_staking:  post.total_for_staking as i128 - pre.total_for_staking as i128,
    total_actually_staked:  post.total_actually_staked as i128 - pre.total_actually_staked as i128,
    to_stake_delta:  post.to_stake_delta as i128 - pre.to_stake_delta as i128,
  
    total_unstaked_and_waiting:  post.total_unstaked_and_waiting as i128 - pre.total_unstaked_and_waiting as i128,
  
    unstake_claims:  post.unstake_claims as i128 - pre.unstake_claims as i128,
    unstake_claims_available_sum:  post.unstake_claims_available_sum as i128 - pre.unstake_claims_available_sum as i128, //how much we have to fulfill unstake claims
  
    staked_in_pools:  post.staked_in_pools as i128 - pre.staked_in_pools as i128,
    unstaked_in_pools:  post.unstaked_in_pools as i128 - pre.unstaked_in_pools as i128,
    total_in_pools:  post.total_in_pools as i128 - pre.total_in_pools as i128,
  
  }
}

//-----------
impl State {

  pub fn test_invariants(&self) -> Result<u8,String> {

    //delta stake must be = delta stake/unstake orders
    if self.total_for_staking>=self.total_actually_staked {
      let delta_stake = self.total_for_staking - self.total_actually_staked;
      if self.epoch_stake_orders < self.epoch_unstake_orders { return Err("delta-stake>0 but self.epoch_stake_orders < self.epoch_unstake_orders".into()) }
      let delta_orders = self.epoch_stake_orders - self.epoch_unstake_orders;
      if delta_stake != delta_orders { return Err("delta_stake!=delta_orders".into())}
    }
    else {
      let delta_unstake = self.total_actually_staked - self.total_for_staking;
      if !(self.epoch_stake_orders < self.epoch_unstake_orders) { return Err("delta-stake NEG but epoch_stake_orders > self.epoch_unstake_orders".into()) }
      let delta_orders = self.epoch_unstake_orders - self.epoch_stake_orders;
      if delta_unstake != delta_orders { return Err("delta_unstake != delta_orders".into()) }
    }
    
    if self.contract_account_balance != self.total_available + self.reserve_for_withdraw + self.epoch_stake_orders {
      return Err("CAB != self.total_available + self.reserve_for_withdraw + self.epoch_stake_orders".into()) 
    }

    return Ok(0);
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
