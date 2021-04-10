//
// MULTI FUN TOKEN [NEP-138](https://github.com/near/NEPs/pull/138)
//

use crate::*;
use near_sdk::{near_bindgen, PromiseResult, log};
use near_sdk::serde::{Deserialize, Serialize};

pub use crate::types::*;
pub use crate::utils::*;

/// one for Each served token
#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct SymbolInfo {
    pub symbol: String,     // token symbol
    pub name: String,       // token name
    pub total_supply: Option<U128String>, //total circulating supply
    pub owner_account_id: Option<String>, // owner of this particular token mint
    pub reference: Option<String>,  // URL to additional resources about the token.
}

//---------INTERACTING CONTRACTS-------------
/// Interface for recipient contract on multi-fungible-token transfers.
#[ext_contract(ext_multifuntok_receiver)]
pub trait ExtMultiFunTokReceiver {

    //NEP-141 single fun token for the default token STNEAR
    fn ft_on_transfer(&mut self, sender_id: AccountId,amount: U128String, msg: String) -> PromiseOrValue<U128>; 

    fn on_multifuntok_transfer(sender_id: AccountId, symbol:String, amount: U128String, memo:String);
}
/// Interface for callback after "on_multifuntok_transfer" to check if the receiving contract executed "on_multifuntok_transfer" ok
#[ext_contract(ext_self_callback)]
pub trait ExtMultiFunTokSelfCallback {

    //NEP-141 single fun token, for the default token
    fn after_ft_on_transfer(&mut self, sender_id:AccountId, contract_id: AccountId, amount: U128String);

    fn after_multifuntok_transfer(sender_id: AccountId, contract_id: AccountId, symbol:String, amount: U128String);
}

#[near_bindgen]
impl MetaPool {

/// NEP-138 Multiple Fungible Tokens Contract
    
    //---------TOKENS---------------

    /// Creates a new Fungible Token 
    /// Requirements:
    /// * Caller can only by the main owner
    pub fn create_token(&mut self, _symbol_info: SymbolInfo){
        panic!("not implemented");
    }

    /// Deletes a Fungible Token 
    /// Requirements:
    /// * Caller can be the main owner or the token owner
    /// * Symbol.total_supply == 0
    pub fn delete_token(&mut self, _symbol: String){
        panic!("not implemented");
    }

    //---------ACCOUNTS---------------

    // Creates an internal `Account` record. Every account has a balance for each one of the served tokens
    // Account created is for `predecessor_id`
    // Requirements:
    // Caller must attach enough NEAR to cover storage cost at the fixed storage price defined in the contract.
    #[payable]
    pub fn create_account(&mut self){
        self.internal_deposit();
    }

    // deletes an account and transfer all balances to beneficiary_id. beneficiary_id must pre-exists if the account holds stnear or META
    // Notes: account_to_delete_id is superfluous on purpose
    // assert!(`account_to_delete_id`==`predecessor_id`)
    pub fn delete_account(&mut self, account_to_delete_id: AccountId, beneficiary_id: AccountId) {
        assert!(env::predecessor_account_id()==account_to_delete_id, "only {} can delete this account",account_to_delete_id.clone());
        let mut acc = self.internal_get_account(&account_to_delete_id);
        assert!(acc.unstaked==0,"you can't delete the account with {} unstake pending",acc.unstaked);
        assert!(acc.nslp_shares==0,"you can't delete the account with {} NSLP shares",acc.nslp_shares);
        let mut beneficiary_acc = self.internal_get_account(&beneficiary_id);
        if acc.available>0 {
            beneficiary_acc.available+=acc.available;
            acc.available = 0;
        }
        if acc.realized_meta>0 {
            beneficiary_acc.realized_meta+=acc.realized_meta;
            acc.realized_meta = 0;
        }
        if acc.stake_shares>0 {
            beneficiary_acc.stake_shares+=acc.stake_shares;
            acc.stake_shares = 0;
        }
        assert!(acc.is_empty(),"inconsistency: account is not empty");
        self.internal_update_account(&account_to_delete_id, &acc);
        self.internal_update_account(&beneficiary_id, &beneficiary_acc);
    }

    /// Transfer `amount` of tok tokens from the caller of the contract (`predecessor_id`) to `receiver_id`.
    /// Requirements:
    /// * receiver_id must pre-exist
    pub fn transfer_to_user(&mut self, receiver_id: AccountId, symbol:String, amount: U128String) {
        self.internal_multifuntok_transfer(&env::predecessor_account_id(), &receiver_id, &symbol, amount.0);
    }

    /// NEP-141 .then() promise 
    /// After Transfer `amount` of symbol tokens to a contract at `receiver_id`.
    /// Check if the contract completed execution of on_multifuntok_transfer
    /// and undo transfer if it failed
    pub fn after_ft_on_transfer(&mut self, sender_id:AccountId, receiver_id: AccountId, amount: U128String) -> U128String {

        assert_callback_calling();
        
        self.contract_busy = false;

        let amount_transferred = amount.into();

        // Get the unused amount from PromiseOrValue result `ft_on_transfer() -> **PromiseOrValue<U128String>**`
        let unused_tokens = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(value) => {
                if let Ok(returned_amount) = near_sdk::serde_json::from_slice::<U128String>(&value) {
                    //max to return is the amount transferred
                    std::cmp::min(amount_transferred, returned_amount.0)
                } else {
                    amount_transferred
                }
            }
            PromiseResult::Failed => {
                log!("call failed. reversing transfer");
                amount_transferred
            },
        };

        if unused_tokens > 0 {
            //some tokens returned / call failed, undo (partially) the transfer 
            self.internal_multifuntok_transfer( &receiver_id, &sender_id, &STNEAR, unused_tokens);
            log!("{} unused tokens returned", unused_tokens);
        }

        //in the current standard, this fn returns "used" amount
        return (amount_transferred - unused_tokens).into();
    }

    /// Transfer `amount` of symbol tokens from the caller of the contract (`predecessor_id`) to a contract at `receiver_id`.
    /// Requirements:
    /// * receiver_id must pre-exist
    /// * receiver_id must be a contract and must respond to `on_multifuntok_transfer(sender_id: AccountId, symbol:String, amount: U128, memo:String)`
    /// * if receiver_id is not a contract or `on_multifuntok_transfer` fails, the transfer is rolled-back
    pub fn transfer_to_contract(&mut self, contract_id: AccountId, symbol:String, amount: U128String, memo:String){

        self.assert_not_busy();
        self.contract_busy = true;

        self.internal_multifuntok_transfer(&env::predecessor_account_id(), &contract_id, &symbol, amount.0);

        ext_multifuntok_receiver::on_multifuntok_transfer(
            env::predecessor_account_id(),
            symbol.clone(),
            amount,
            memo,
            //promise params:
            &contract_id, //contract
            0, //attached native NEAR amount
            100*TGAS,
        )
        .then(ext_self_callback::after_multifuntok_transfer(
            env::predecessor_account_id(),
            contract_id,
            symbol.clone(),
            amount,
            //promise params:
            &env::current_account_id(),//contract
            0, //attached native NEAR amount
            30*TGAS, //30TGAS
        ));

    }

    /// After Transfer `amount` of symbol tokens to a contract at `receiver_id`.
    /// Check if the contract completed execution of on_multifuntok_transfer
    /// and undo transfer if it failed
    #[private]
    pub fn after_multifuntok_transfer(&mut self, sender_id:AccountId, contract_id: AccountId, symbol:String, amount: U128String){

        assert_callback_calling();

        self.contract_busy = false;

        if !is_promise_success() {
            //undo transfer
            self.internal_multifuntok_transfer( &contract_id, &sender_id, &symbol, amount.0);
            env::log("transfer to contract failed".as_bytes());
        }
    }


    //---------VIEW METHODS-------------

    /// return the list of all tokens this contract serves
    pub fn get_symbols(&self) -> Vec<SymbolInfo>{
        return vec!(
            SymbolInfo {
                symbol:"NEAR".into(),
                name:"native NEAR".into(),
                total_supply:None,
                owner_account_id:None,
                reference:Some("near.org".into()),
            },
            SymbolInfo {
                symbol:STNEAR.into(),
                name:"div-pool staked near".into(),
                total_supply: Some(self.total_for_staking.into()),
                owner_account_id: Some(env::current_account_id()),
                reference: Some("www.narwallets.com".into()),
            },
            SymbolInfo {
                symbol:"META".into(),
                name:"div-pool governance token".into(),
                total_supply: Some(self.total_meta.into()),
                owner_account_id: Some(env::current_account_id()),
                reference: Some("www.narwallets.com".into()),
            },
        )
    }

    /// Returns info & total supply of tokens of a symbol
    pub fn get_symbol(&self, symbol:String) -> SymbolInfo {
        let inx:usize = match &symbol as &str {
            "NEAR"=>0, STNEAR=>1, "META"=>2, _=>panic!("invalid symbol")
        };
        return self.get_symbols()[inx].clone();
    }

    /// Checks if account already exists
    pub fn account_exists(&self, account_id:AccountId) -> bool {
        return !self.internal_get_account(&account_id).is_empty();
    }

    /// Returns balance of the `owner_id` account & token.
    pub fn get_funtok_balance(&self, account_id: AccountId, symbol:String) -> U128String {
        let acc = self.internal_get_account(&account_id);
        let amount:u128 = match &symbol as &str {
            "NEAR"=>acc.available ,
            STNEAR=>self.amount_from_stake_shares(acc.stake_shares), 
            "META"=>acc.total_meta(self), 
            _=>panic!("invalid symbol")
        };
        return amount.into();
    }

}
