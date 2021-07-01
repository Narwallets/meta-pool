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

use metapool::*;

/// Helper to log ExecutionResult outcome of a call/view
pub fn print_exec_result_single(res: &ExecutionResult) {
    let is_ok = res.is_ok();

    for line in &res.outcome().logs {
        if !is_ok && line.starts_with("{\"") {
            //add a prefix to event lines if the transaction failed
            println!("(failed) {:?}", line);
        } else {
            println!("{:?}", line);
        }
    }
    if !is_ok {
        println!("{:?}", res);
    }
}
/// Helper to log ExecutionResult outcome of a call/view
pub fn print_exec_result_promise(inx: u64, res: &ExecutionResult) {
    if res.outcome().logs.len() == 0 || res.is_ok() {
        return;
    }
    println!("--promise #{}", inx);
    print_exec_result_single(&res);
}

pub fn print_exec_result(res: &ExecutionResult) {
    print_exec_result_single(&res);
    let mut inx = 0;
    for pr in &res.promise_results() {
        if let Some(some_pr) = pr {
            print_exec_result_promise(inx, &some_pr);
            inx += 1;
        }
    }
}

pub fn print_logs(res: &ExecutionResult) {
    for item in &res.promise_results() {
        if let Some(some_res) = item {
            for line in &some_res.outcome().logs {
                println!("{:?}", line);
            }
        }
    }
}

pub fn check_exec_result_single(res: &ExecutionResult) {
    //println!("Result: {:#?}", res);
    for line in &res.outcome().logs {
        println!("{:?}", line);
    }
    if !res.is_ok() {
        println!("{:?}", res);
    }
    assert!(res.is_ok());
}

pub fn check_exec_result(res: &ExecutionResult) {
    check_exec_result_single(res);
    for pr in &res.promise_results() {
        if let Some(some_pr) = pr {
            check_exec_result_single(&some_pr);
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

pub fn print_vec_u8(title: &str, v: &Vec<u8>) {
    println!(
        "{}:{}",
        title,
        match std::str::from_utf8(v) {
            Ok(v) => v,
            Err(_) => "[[can't decode result, invalid UFT8 sequence]]",
        }
    )
}

pub fn ntoy(near: u64) -> u128 {
    to_yocto(&near.to_string())
}
#[allow(non_snake_case)]
pub fn ntoU128(near: u64) -> String {
    ntoy(near).to_string()
}

pub fn yton(yoctos: u128) -> String {
    let mut str = format!("{:0>25}", yoctos);
    let dec = str.split_off(str.len() - 24);
    return [&str, ".", &dec].concat();
}

//----------------------
pub fn view(contract_account: &UserAccount, method: &str, args_json: &str) -> Value {
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

pub fn as_u128(v: &Value) -> u128 {
    return match v.as_str() {
        Some(x) => {
            //println!("{}",x);
            x.parse::<u128>().unwrap()
        }
        _ => panic!("invalid u128 value {:#?}", v),
    };
}
pub fn view_u128(contract_account: &UserAccount, method: &str, args_json: &str) -> u128 {
    let result = view(contract_account, method, args_json);
    return as_u128(&result);
}

//----------------------
pub fn call(
    who: &UserAccount,
    contract_account: &UserAccount,
    method: &str,
    args_json: &str,
    attached_deposit: u128,
    gas: u64,
) -> ExecutionResult {
    // let pct = PendingContractTx {
    //   receiver_id: contract_account.account_id(),
    //   method: method.into(),
    //   args: args_json.into(),
    //   is_view:false,
    // };
    let exec_res = who.call(
        contract_account.account_id(),
        method,
        args_json.as_bytes(),
        gas,
        attached_deposit,
    );
    //println!("Result: {:#?}", exec_res);
    return exec_res;
}

#[allow(dead_code)]
pub fn show_balance(ua: &UserAccount) {
    println!("@{} balance: {}", ua.account_id(), balance(ua));
}

pub fn assert_less_than_one_milli_near_diff_balance(
    action: &str,
    bal: u128,
    expected: u128,
) -> bool {
    if bal == expected {
        return true;
    };
    if bal > expected {
        panic!(
            "{} failed MORE THAN EXPECTED diff:{} bal:{} expected:{}",
            action,
            yton(bal - expected),
            yton(bal),
            yton(expected)
        );
    }
    let differ = expected - bal;
    if differ < ONE_MILLI_NEAR {
        return true;
    };
    panic!(
        "{} failed LESS THAN EXPECTED by more than 0.001 diff:{} bal:{} expected:{}",
        action,
        yton(differ),
        yton(bal),
        yton(expected)
    );
}

pub fn balance(acc: &UserAccount) -> u128 {
    if let Some(data) = acc.account() {
        data.amount + data.locked
    } else {
        0
    }
}
