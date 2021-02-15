use crate::*;
use near_sdk::serde::{Deserialize, Serialize};

pub use crate::types::*;
pub use crate::utils::*;

//------------------------
//  Validator's Loan Req Status
//------------------------
pub const DRAFT: u8 = 0;
pub const ACTIVE: u8 = 1;
pub const REJECTED: u8 = 2;
pub const APPROVED: u8 = 3;
pub const FEE_PAID: u8 = 4;
pub const EXECUTING: u8 = 5;
pub const COMPLETED: u8 = 6;

const ACTIVATION_FEE:u128= 5*NEAR;
const MIN_REQUEST:u128 = 10*K_NEAR;

//------------------------
//  Validator's Loan Req
//------------------------
#[derive(BorshDeserialize, BorshSerialize)]
#[serde(crate = "near_sdk::serde")]
#[derive(Serialize, Deserialize)]
pub struct VLoanRequest {
    //total requested
    pub amount_requested: u128,

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
    pub loan_fee: u128,

    //EpochHeight where the request was activated status=active
    pub activated_epoch_height: EpochHeight,
}

#[serde(crate = "near_sdk::serde")]
#[derive(Serialize, Deserialize)]
pub struct VLoanInfo {
    //same as above but u128 => U128String so the json ser/deser does the right thing

    //total requested
    pub amount_requested: U128String,

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
    pub loan_fee: U128String,

    //EpochHeight where the request was activated status=active
    pub activated_epoch_height: U64String,
}

impl Default for VLoanRequest {
    fn default() -> Self {
        Self {
            amount_requested: 0,
            staking_pool_account_id: String::from(""),
            information_url: String::from(""),
            commited_fee: 0,
            commited_fee_duration: 0,
            status: DRAFT,
            loan_fee: 0,
            activated_epoch_height: 0,
        }
    }
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct VLoanRequestInfo {
    //same as above but with data-types compatible with serde-json
    pub amount_requested: U128String,
    pub staking_pool_account_id: AccountId,
    pub information_url: String,
    pub commited_fee: u16,
    pub commited_fee_duration: u16,
    pub status: u8,
    pub loan_fee_near: U128String,
    pub activated_epoch_height: U64String,
}

#[near_bindgen]
impl MetaPool {

    /// get loan_request
    pub fn get_vloan_request(&self, account_id: AccountId) -> VLoanInfo {
        let request = self.loan_requests.get(&account_id).unwrap_or_default();
        return VLoanInfo {
            status: request.status,
            amount_requested: request.amount_requested.into(),
            staking_pool_account_id: request.staking_pool_account_id,
            information_url: request.information_url,
            commited_fee: request.commited_fee,
            commited_fee_duration: request.commited_fee_duration,
            loan_fee: request.loan_fee.into(),
            activated_epoch_height: request.activated_epoch_height.into(),
        };
    }

    /// update a loan_request
    pub fn set_vloan_request(
        &mut self,
        amount_requested: U128String,
        staking_pool_account_id: String,
        commited_fee: u16,
        commited_fee_duration: u16,
        information_url: String
    ) {
        let mut request = self.loan_requests.get(&env::predecessor_account_id()).unwrap_or_default();
        //check status transition
        //check status 
        assert!(request.status==DRAFT,"You can only modify DRAFT requests");
        request.staking_pool_account_id = staking_pool_account_id;
        request.amount_requested = amount_requested.0;
        request.commited_fee = commited_fee;
        request.commited_fee_duration = commited_fee_duration;
        request.information_url = information_url;
        
        self.loan_requests.insert(&env::predecessor_account_id(), &request);
    }

    // activate a loan_request
    #[payable]
    pub fn vloan_activate(&mut self){
        //get request
        let mut request = self.loan_requests.get(&env::predecessor_account_id()).unwrap_or_default();
        //check status 
        assert!(request.status==DRAFT,"You can only activate DRAFT requests");
        //check fee
        assert!(env::attached_deposit()==ACTIVATION_FEE,"Activation fee MUST be {}",ACTIVATION_FEE);
        assert!(env::is_valid_account_id(&request.staking_pool_account_id.as_bytes()),"invalid staking pool account id");
        assert!(request.amount_requested >= MIN_REQUEST, "Min amount is {}",MIN_REQUEST);
        assert!(request.commited_fee<2000,"invalid commited fee. Maxc 20%");
        assert!(request.commited_fee_duration>0,"invalid commited fee duration");
        request.activated_epoch_height = env::epoch_height();
        //update
        self.loan_requests.insert(&env::predecessor_account_id(), &request);
        //consume fee
        self.internal_deposit_attached_near_into(self.treasury_account_id.clone());
    }   

    //deactivate a loan request
    pub fn vloan_convert_back_to_draft(&mut self){
        //check status
        let mut request = self.loan_requests.get(&env::predecessor_account_id()).unwrap_or_default();
        //check time
        assert!(env::epoch_height()>=request.activated_epoch_height+1,"you must wait 2 Epochs to deactivate a request");
        //check status transition
        match request.status {
            ACTIVE|APPROVED|COMPLETED => { request.status = DRAFT },
            _ => { panic!("You can only convert to DRAFT if status is ACTIVE|APPROVED|COMPLETED") }
        }
        //update
        self.loan_requests.insert(&env::predecessor_account_id(), &request);
    }   

    pub fn vloan_delete(&mut self){
        //check status
        let mut request = self.loan_requests.get(&env::predecessor_account_id()).unwrap_or_default();
        //check time
        match request.status {
            DRAFT|COMPLETED => { request.status = DRAFT },
            _ => { panic!("You can only delete the request if the status is DRAFT|COMPLETED. Deactivate it first") }
        }
        //delete
        self.loan_requests.remove(&env::predecessor_account_id());
    }   

    #[payable]
    pub fn vloan_take(&mut self){
        let mut request = self.loan_requests.get(&env::predecessor_account_id()).unwrap_or_default();
        //check status
        assert!(request.status==APPROVED,"Request is not APPROVED");
        //check fee
        assert!(env::attached_deposit()==request.loan_fee,"Attached Loan Fee MUST be {}",request.loan_fee);
        request.status = FEE_PAID;
        //update
        self.loan_requests.insert(&env::predecessor_account_id(), &request);
        //consume fee
        self.internal_deposit_attached_near_into(self.treasury_account_id.clone());
    }   
}
