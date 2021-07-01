#![allow(unused_imports)]
#![allow(dead_code)]

use near_sdk_sim::{
    account::AccessKey,
    call, deploy, init_simulator,
    near_crypto::{KeyType, SecretKey, Signer},
    to_yocto, view, ContractAccount, ExecutionResult, UserAccount, ViewResult, DEFAULT_GAS,
    STORAGE_AMOUNT,
};

use near_sdk::serde_json::Value;

use crate::sim_utils::*;
use metapool::*;

pub const SP_INITIAL_BALANCE: u128 = 36 * NEAR;

// Load contracts' bytes.
near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
  WASM_BYTES_META_POOL => "../res/metapool.wasm",
  //static ref WASM_BYTES_META_POOL: &'static [u8] = include_bytes!("../../res/metapool.wasm").as_ref();
  WASM_BYTES_SP => "../res/staking_pool.wasm",
  // static ref WASM_BYTES_SP: &'static [u8] = include_bytes!("../../res/staking_pool.wasm").as_ref();
  WASM_BYTES_GET_EPOCH => "../res/get_epoch_contract.wasm",
  // static ref WASM_BYTES_GET_EPOCH: &'static [u8] = include_bytes!("../../res/get_epoch_contract.wasm").as_ref();
}

/// Deploy the contract(s) and create some metapool accounts. Returns:
/// - The metapool Contract
/// - Root Account
/// - Testnet Account (utility suffix for building other addresses)
/// - A deployer account address
//Note: MetaPoolContract is a struct "magically" created by #[near_bindgen] (near_skd_rs~2.0.4)
/*
pub fn init_simulator_and_contract(
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
*/

//-----------------------
fn deploy_simulated_staking_pool(
    master_account: &UserAccount,
    deploy_to_acc_id: &str,
    owner_account_id: &str,
) -> UserAccount {
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
    check_exec_result(&res);
    return sp;
}

const METAPOOL_CONTRACT_ID: &str = "metapool";
//-----------------------------
//-----------------------------
//-----------------------------
pub struct Simulation {
    pub metapool: ContractAccount<MetaPoolContract>,
    pub master_account: UserAccount, // root
    pub testnet: UserAccount,        // testnet suffix
    pub owner: UserAccount,          // deployer account
    pub treasury: UserAccount,
    pub operator: UserAccount,
    pub sp: Vec<UserAccount>,       //Staking pools
    pub get_epoch_acc: UserAccount, //contract to get env::epoch_height()
    pub weight_basis_points_vec: Vec<u16>,
}
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
        init_method:new(owner.account_id(), treasury.account_id(), operator.account_id(), "meta_token_contract_account".into())
        );

        //deploy all the staking pools
        let mut sp = Vec::with_capacity(4);
        //---- and register staking pools in the metapool contract
        let weight_basis_points_vec = vec![15, 40, 25, 20];
        //----
        for n in 0..=3 {
            let acc_id = &format!("sp{}", n);
            let sp_contract =
                deploy_simulated_staking_pool(&master_account, acc_id, &owner.account_id());
            //call(&owner,&sp_contract,"pause_staking","{}",0,10*TGAS);
            sp.push(sp_contract);
            //-- register in the staking pool
            call!(
                owner,
                metapool.set_staking_pool(acc_id.clone(), weight_basis_points_vec[n] * 100),
                gas = 25 * TGAS
            );
        }

        let total_w_bp = view!(metapool.sum_staking_pool_list_weight_basis_points());
        assert!(total_w_bp.unwrap_json_value() == 10000);

        //deploy a contract to get the current epoch
        let get_epoch_acc = master_account.deploy(
            &WASM_BYTES_GET_EPOCH,
            String::from("get_epoch_acc"),
            SP_INITIAL_BALANCE,
        );
        master_account
            .create_transaction(get_epoch_acc.account_id())
            .function_call("new".into(), "{}".into(), 50 * TGAS, 0)
            .submit();

        return Self {
            metapool,

            master_account,

            testnet,
            owner,
            treasury,
            operator,

            sp,

            get_epoch_acc,
            weight_basis_points_vec,
        };
    }

    pub fn sp_staked(&self, n: usize) -> u128 {
        view_u128(
            &self.sp[n],
            "get_account_staked_balance",
            &format!(r#"{{"account_id":"{}"}}"#, METAPOOL_CONTRACT_ID),
        )
    }

    // pub fn sp_balance(&self, n:usize) -> u128 {
    //   if let Some(data) = self.sp[n].account() {
    //     data.amount+data.locked
    //   }
    //   else { 0 }
    //   //self.sp[n].amount()+self.sp[n].locked()
    // }

    pub fn show_sp_staked_balance(&self, n: usize) {
        //let total = self.sp_balance(n);
        //let data = self.sp[n].account().unwrap();
        //println!("{}",&format!(r#"{{"account_id":"{}"}}"#,&METAPOOL_CONTRACT_ID));
        println!("sp{} native acc {:?}", n, &self.sp[n].account().unwrap());
        let staked = view_u128(
            &self.sp[n],
            "get_account_staked_balance",
            &format!(r#"{{"account_id":"{}"}}"#, METAPOOL_CONTRACT_ID),
        );
        //println!("sp{} get_account_staked_balance:{}, data.amount:{}+data.locked:{}", n, yton(staked));//, data.amount, data.locked );
        println!("sp{} get_account_staked_balance:{}", n, yton(staked)); //, data.amount, data.locked );
    }

    pub fn show_sps_staked_balances(&self) {
        println!("--SPs balance");
        for n in 0..=3 {
            self.show_sp_staked_balance(n)
        }
        println!("--------------");
    }

    //----------------
    pub fn show_account_info(&self, acc: &str) -> Value {
        let metapool = &self.metapool;
        let result = view!(metapool.get_account_info(acc.into()));
        print_vec_u8(acc, &result.unwrap());
        //println!("Result: {:#?}", result.unwrap_json_value());
        return near_sdk::serde_json::from_str(std::str::from_utf8(&result.unwrap()).unwrap())
            .unwrap();
    }
}
