use crate::*;
use near_sdk::serde::{Deserialize, Serialize};

pub use crate::types::*;
pub use crate::utils::*;


//------------------------
//  Validator's Loan Req Status
//------------------------
pub const DRAFT:u8=0;
pub const ACTIVE:u8=1;
pub const REJECTED:u8=2;
pub const ACCEPTED:u8=3;
pub const EXECUTED:u8=4;

//------------------------
//  Validator's Loan Req
//------------------------
#[derive(BorshDeserialize, BorshSerialize)]
#[serde(crate = "near_sdk::serde")]
#[derive(Serialize, Deserialize)]
pub struct VLoanRequest {

    //total requested 
    pub amount_requested_near: u128,

    //staking pool beneficiary
    pub staking_pool_account_id: AccountId,

    //more information 
    pub information_url: String,

    //committed fee
    //The validator commits to have their fee at x%, x amount of epochs
    //100 => 1% , 250=>2.5%, etc. -- max: 10000=>100%
    pub commited_fee: u16,
    pub commited_fee_duration: u16,

    //status: set by requester: draft, active / set by owner: rejected, accepted, implemented
    pub status: u8,
    //set by owner. if status=accepted how much will be taken from the user account as fee to move to status=implemented
    pub loan_fee_near: u128,

    //EpochHeight where the request was activated status=active
    pub activated_epoch_height: EpochHeight,

}

impl Default for VLoanRequest {
    fn default() -> Self {
        Self {
            amount_requested_near: 0,
            staking_pool_account_id: String::from(""),
            information_url: String::from(""),
            commited_fee: 0,
            commited_fee_duration:0,
            status: DRAFT,
            loan_fee_near: 0,
            activated_epoch_height: 0,
        }
    }
}

// get_staking_pools_list returns StakingPoolJSONInfo[]
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct VLoanRequestInfo {  //same as above but with data-types compatible with serde-json
    pub amount_requested_near: U128String,
    pub staking_pool_account_id: AccountId,
    pub information_url: String,
    pub commited_fee: u16,
    pub commited_fee_duration: u16,
    pub status: u8,
    pub loan_fee_near: U128String,
    pub activated_epoch_height: U64String,
}

#[near_bindgen]
impl DiversifiedPool {

    /// create or update a loan_request
    pub fn set_loan_request(&mut self, amount_requested:U128String, commited_fee:u16, commited_fee_duration:u16, information_url: String){
        /*let mut request = self.loan_requests.get(&env::predecessor_account_id()).unwrap_or_default();
        request.amount_requested_near = amount_requested.0;
        request.commited_fee = commited_fee;
        request.commited_fee_duration = commited_fee_duration;
        request.information_url = information_url;
        self.loan_requests.insert(&env::predecessor_account_id(), &request);
        */
    }

}
