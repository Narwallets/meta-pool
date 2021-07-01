#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
///
/// fuzzy tests
///
/// Mechanism:
/// ---------
/// create n users
/// chose a pseudo-random seed
/// according to rand, make one user perform an action
/// it can be the bot performing distribute, retrieve, clearing
/// repeat until 50 actions or errors
/// check & fix bugs.
/// try other seeds, record useful seeds (complex patterns, low % of errors)
///
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

use rand::{Rng, SeedableRng};
use rand_pcg::Pcg32;

use crate::sim_contract_state::*;
use crate::sim_setup::*;
use crate::sim_steps::*;
use crate::sim_utils::*;
use metapool::*;

const COUNT_USERS: usize = 10;

#[derive(Debug)]
pub enum Action {
    Stake,
    LiquidUnstake,
    DelayedUnstake,
    DUWithdraw,
    AddLiquidity,
    RemoveLiquidity,
    BotDistributes,
    BotEndOfEpochClearing,
    BotRetrieveFunds,
    BotPingRewards,
    LastAction,
}

impl Action {
    fn from_u8(n: u8) -> Option<Action> {
        if n < Action::LastAction as u8 {
            Some(unsafe { std::mem::transmute(n) })
        } else {
            None
        }
    }
}

//-----------
pub fn step_random_action(
    sim: &Simulation,
    acc: &UserAccount,
    action: Action,
    amount_near: u64,
    pre: &State,
) -> Result<StateAndDiff, String> {
    println!("step_random_action {:?} {}", action, amount_near);

    return match action {
        Action::Stake => step_call(
            &sim,
            &acc,
            "deposit_and_stake",
            json!({}),
            50 * TGAS,
            amount_near as u128 * NEAR,
            &pre,
        ),
        Action::AddLiquidity => step_call(
            &sim,
            &acc,
            "nslp_add_liquidity",
            json!({}),
            200 * TGAS,
            amount_near as u128 * NEAR,
            &pre,
        ),
        Action::RemoveLiquidity => step_call(
            &sim,
            &acc,
            "nslp_remove_liquidity",
            json!({ "amount": ntoU128(amount_near) }),
            200 * TGAS,
            NO_DEPOSIT,
            &pre,
        ),
        Action::DelayedUnstake => step_call(
            &sim,
            &acc,
            "unstake",
            json!({ "amount": ntoU128(amount_near) }),
            100 * TGAS,
            NO_DEPOSIT,
            &pre,
        ),
        Action::DUWithdraw => step_call(
            &sim,
            &acc,
            "withdraw",
            json!({ "amount": ntoU128(amount_near) }),
            50 * TGAS,
            NO_DEPOSIT,
            &pre,
        ),
        Action::LiquidUnstake => step_call(
            &sim,
            &acc,
            "liquid_unstake",
            json!({"stnear_to_burn": ntoU128(amount_near), "min_expected_near": ntoU128(amount_near*95/100)}),
            50 * TGAS,
            NO_DEPOSIT,
            &pre,
        ),
        Action::BotDistributes => bot_distributes(&sim, &pre),
        Action::BotEndOfEpochClearing => bot_end_of_epoch_clearing(&sim, &pre),
        Action::BotRetrieveFunds => bot_retrieve(&sim, &pre),
        Action::BotPingRewards => bot_ping_rewards(&sim, &pre),
        Action::LastAction => panic!("invalid action"),
    };
}

const SEED_COUNT: u16 = 5;
const START_SEED: u16 = 0;
const END_SEED: u16 = START_SEED + SEED_COUNT;

#[derive(Debug)]
struct SeedResults {
    seed: u64,
    steps_ok: u16,
}

#[test]
fn simulation_fuzzy() {
    let mut seed_results: Vec<SeedResults> = Vec::with_capacity(SEED_COUNT as usize);

    for seed in START_SEED..END_SEED {
        println!("//----------------");
        println!("// -- Start seed = {}", seed);
        println!("//----------------");

        let mut rng = Pcg32::seed_from_u64(seed as u64);
        // for _ in 0..50 {
        //     let y: u8 = rng.gen_range(0..10);
        //     println!("{} {}", x, y);
        // }

        let sim = Simulation::new();

        let metapool = &sim.metapool;

        //---- Users
        let mut users: Vec<UserAccount> = Vec::with_capacity(COUNT_USERS);
        for n in 0..COUNT_USERS {
            users.push(sim.testnet.create_user(format!("user{}", n), ntoy(500_000)));
        }

        let pre = build_state(&sim);
        // initial stake
        println!("--PRE {}", serde_json::to_string(&pre).unwrap_or_default());

        let amount_add_liq = 100_000 * NEAR;
        let amount_stake = 190_000 * NEAR;
        // user0 adds liquidity
        let mut initial_commands = step_call(
            &sim,
            &users[0],
            "nslp_add_liquidity",
            json!({}),
            200 * TGAS,
            amount_add_liq,
            &pre,
        )
        .unwrap();
        //everyone stakes
        for n in 0..COUNT_USERS {
            initial_commands = step_call(
                &sim,
                &users[n],
                "deposit_and_stake",
                json!({}),
                200 * TGAS,
                amount_stake,
                &initial_commands.state,
            )
            .unwrap()
        }

        let mut count_steps: u16 = 0;
        let mut count_steps_ok: u16 = 0;
        let mut state: State = initial_commands.state;
        // let mut diff: StateDiff;

        //50 fuzzy steps for each seed
        for fuzzy_steps in 1..50 {
            //--------------------
            //choose a random user
            //--------------------
            let user: usize = rng.gen_range(0..COUNT_USERS);

            count_steps += 1;
            println!("//---seed-step {}.{}", seed, count_steps);

            //--------------------
            //choose a random action
            //--------------------
            const COUNT_ACTIONS: usize = Action::LastAction as usize;
            let action_index: u8 = rng.gen_range(0..COUNT_ACTIONS) as u8;
            let action = Action::from_u8(action_index).unwrap();
            println!("random action {} {:?}", action_index, action);

            //---------------------
            //choose a random amount
            //---------------------
            let amount_u16: u16 = rng.gen();
            let amount_near = amount_u16 as u64;

            match step_random_action(&sim, &users[user], action, amount_near, &state) {
                Err(x) => println!("{}", x),
                Ok(data) => {
                    state = data.state;
                    count_steps_ok += 1;
                }
            }
        } //50 fuzzy steps

        seed_results.push(SeedResults {
            seed: seed.into(),
            steps_ok: count_steps_ok,
        });

        //ordered end of epoch
        let _r0 = bot_ping_rewards(&sim, &state);
        let _r1 = bot_distributes(&sim, &state);
        let _r2 = bot_retrieve(&sim, &state);
        let result = bot_end_of_epoch_clearing(&sim, &state);

        //after orderly end_of_epoch check stricter invariants
        if let Ok(res) = &result {
            if res.state.epoch_stake_orders != 0 || res.state.epoch_unstake_orders != 0 {
                // both must be 0 after an orderly end_of_epoch
                panic!("after orderly end_of_epoch_clearing epoch_stake_orders {} epoch_unstake_orders {}",res.state.epoch_stake_orders,res.state.epoch_unstake_orders)
            }
            //no delta should remain
            if res.state.to_stake_delta != 0 {
                panic!("after orderly end_of_epoch_clearing epoch_stake_orders {} epoch_unstake_orders {}",res.state.epoch_stake_orders,res.state.epoch_unstake_orders)
            }
        }
    } // x seeds

    println!("{:?}", seed_results);
}
