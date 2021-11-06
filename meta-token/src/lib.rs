use near_sdk::collections::LookupMap;

use near_contract_standards::fungible_token::{
    core::FungibleTokenCore,
    metadata::{FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC},
    resolver::FungibleTokenResolver,
};

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LazyOption;
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::{
    assert_one_yocto, env, ext_contract, log, near_bindgen, AccountId, Balance, Gas,
    PanicOnDefault, PromiseOrValue,
};

//-- Sputnik DAO remote upgrade requires BLOCKCHAIN_INTERFACE low-level access
#[cfg(target_arch = "wasm32")]
use near_sdk::env::BLOCKCHAIN_INTERFACE;

const TGAS: Gas = 1_000_000_000_000;
const GAS_FOR_RESOLVE_TRANSFER: Gas = 5 * TGAS;
const GAS_FOR_FT_TRANSFER_CALL: Gas = 25 * TGAS;
const NO_DEPOSIT: Balance = 0;

// nanoseconds in a second
const NANOSECONDS: u64 = 1_000_000_000;

type U128String = U128;

near_sdk::setup_alloc!();

mod internal;
mod migrations;
mod storage_nep_145;
mod util;
mod vesting;

use util::*;
use vesting::{VestingRecord, VestingRecordJSON};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct MetaToken {
    metadata: LazyOption<FungibleTokenMetadata>,

    pub accounts: LookupMap<AccountId, Balance>,

    pub owner_id: AccountId,

    pub minters: Vec<AccountId>,

    pub total_supply: Balance,

    /// transfers are locked until this moment
    pub locked_until_nano: TimestampNano,

    pub vested: LookupMap<AccountId, VestingRecord>,
    pub vested_count: u32,
}

#[near_bindgen]
impl MetaToken {
    /// Initializes the contract with the given total supply owned by the given `owner_id`.

    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        //validate default metadata
        internal::default_ft_metadata().assert_valid();
        Self {
            owner_id: owner_id.clone(),
            metadata: LazyOption::new(b"m".to_vec(), None),
            accounts: LookupMap::new(b"a".to_vec()),
            minters: vec![owner_id],
            total_supply: 0,
            locked_until_nano: 0,
            vested: LookupMap::new(b"v".to_vec()),
            vested_count: 0,
        }
    }

    /// Returns account ID of the owner.
    pub fn get_owner_id(&self) -> AccountId {
        return self.owner_id.clone();
    }
    pub fn set_owner_id(&mut self, owner_id: AccountId) {
        self.assert_owner_calling();
        assert!(env::is_valid_account_id(owner_id.as_bytes()));
        self.owner_id = owner_id.into();
    }
    pub fn set_locked_until(&mut self, unix_timestamp: u32) {
        self.assert_owner_calling();
        self.locked_until_nano = unix_timestamp as u64 * NANOSECONDS;
    }

    // whitelisted minters can mint more into some account
    #[payable]
    pub fn mint(&mut self, account_id: &AccountId, amount: U128String) {
        assert_one_yocto();
        self.assert_minter(env::predecessor_account_id());
        self.mint_into(account_id, amount.0);
    }

    //owner can add/remove minters
    #[payable]
    pub fn add_minter(&mut self, account_id: AccountId) {
        assert_one_yocto();
        self.assert_owner_calling();
        if let Some(_) = self.minters.iter().position(|x| *x == account_id) {
            //found
            panic!("already in the list");
        }
        self.minters.push(account_id);
    }

    #[payable]
    pub fn remove_minter(&mut self, account_id: &AccountId) {
        assert_one_yocto();
        self.assert_owner_calling();
        if let Some(inx) = self.minters.iter().position(|x| x == account_id) {
            //found
            let _removed = self.minters.swap_remove(inx);
        } else {
            panic!("not a minter")
        }
    }

    pub fn get_minters(self) -> Vec<AccountId> {
        self.minters
    }

    /// sets metadata icon
    #[payable]
    pub fn set_metadata_icon(&mut self, svg_string: String) {
        assert_one_yocto();
        self.assert_owner_calling();
        let mut m = self.internal_get_ft_metadata();
        m.icon = Some(svg_string);
        self.metadata.set(&m);
    }

    /// sets metadata_reference
    #[payable]
    pub fn set_metadata_reference(&mut self, reference: String, reference_hash: String) {
        assert_one_yocto();
        self.assert_owner_calling();
        let mut m = self.internal_get_ft_metadata();
        m.reference = Some(reference);
        m.reference_hash = Some(reference_hash.as_bytes().to_vec().into());
        m.assert_valid();
        self.metadata.set(&m);
    }

    //-----------
    //-- Vesting functions in the contract
    //-----------
    /// Get the amount of tokens that are locked in this account due to lockup or vesting.
    pub fn get_locked_amount(&self, account: AccountId) -> U128String {
        match self.vested.get(&account) {
            Some(vesting) => vesting.compute_amount_locked().into(),
            None => 0.into(),
        }
    }

    /// Get vesting information
    pub fn get_vesting_info(&self, account_id: AccountId) -> VestingRecordJSON {
        log!("{}", &account_id);
        let vesting = self.vested.get(&account_id).unwrap();
        VestingRecordJSON {
            amount: vesting.amount.into(),
            locked_until_timestamp: vesting.locked_until_timestamp.into(),
            linear_start_timestamp: vesting.linear_start_timestamp.into(),
            linear_end_timestamp: vesting.linear_end_timestamp.into(),
        }
    }

    //minters can mint with vesting/locked periods
    #[payable]
    pub fn mint_vested(
        &mut self,
        account_id: &AccountId,
        amount: U128String,
        locked_until_timestamp: U64String,
        linear_start_timestamp: U64String,
        linear_end_timestamp: U64String,
    ) {
        self.mint(account_id, amount);
        let record = VestingRecord::new(
            amount.into(),
            locked_until_timestamp.into(),
            linear_start_timestamp.into(),
            linear_end_timestamp.into(),
        );
        match self.vested.insert(&account_id, &record) {
            Some(_) => panic!("account already vested"),
            None => self.vested_count += 1,
        }
    }

    #[payable]
    /// terminate vesting before is over
    /// burn the tokens
    pub fn terminate_vesting(&mut self, account_id: &AccountId) {
        assert_one_yocto();
        self.assert_owner_calling();
        match self.vested.get(&account_id) {
            Some(vesting) => {
                let locked_amount = vesting.compute_amount_locked();
                if locked_amount == 0 {
                    panic!("locked_amount is zero")
                }
                self.internal_burn(account_id, locked_amount);
                self.vested.remove(&account_id);
                self.vested_count -= 1;
                log!(
                    "{} vesting terminated, {} burned",
                    account_id,
                    locked_amount
                );
            }
            None => panic!("account not vested"),
        }
    }

    /// return how many vested accounts are still active
    pub fn vested_accounts_count(&self) -> u32 {
        self.vested_count
    }

    //---------------------------------------------------------------------------
    /// Sputnik DAO remote-upgrade receiver
    /// can be called by a remote-upgrade proposal
    ///
    #[cfg(target_arch = "wasm32")]
    pub fn upgrade(self) {
        assert!(env::predecessor_account_id() == self.owner_id);
        //input is code:<Vec<u8> on REGISTER 0
        //log!("bytes.length {}", code.unwrap().len());
        const GAS_FOR_UPGRADE: u64 = 10 * TGAS; //gas occupied by this fn
        const BLOCKCHAIN_INTERFACE_NOT_SET_ERR: &str = "Blockchain interface not set.";
        //after upgrade we call *pub fn migrate()* on the NEW CODE
        let current_id = env::current_account_id().into_bytes();
        let migrate_method_name = "migrate".as_bytes().to_vec();
        let attached_gas = env::prepaid_gas() - env::used_gas() - GAS_FOR_UPGRADE;
        unsafe {
            BLOCKCHAIN_INTERFACE.with(|b| {
                // Load input (new contract code) into register 0
                b.borrow()
                    .as_ref()
                    .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                    .input(0);

                //prepare self-call promise
                let promise_id = b
                    .borrow()
                    .as_ref()
                    .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                    .promise_batch_create(current_id.len() as _, current_id.as_ptr() as _);

                //1st action, deploy/upgrade code (takes code from register 0)
                b.borrow()
                    .as_ref()
                    .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                    .promise_batch_action_deploy_contract(promise_id, u64::MAX as _, 0);

                //2nd action, schedule a call to "migrate()".
                //Will execute on the **new code**
                b.borrow()
                    .as_ref()
                    .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                    .promise_batch_action_function_call(
                        promise_id,
                        migrate_method_name.len() as _,
                        migrate_method_name.as_ptr() as _,
                        0 as _,
                        0 as _,
                        0 as _,
                        attached_gas,
                    );
            });
        }
    }
}

//----------------------------------------------
// ft metadata standard
// Q: Is ignoring storage costs the only reason for the re-implementation?
// A: making the user manage storage costs adds too much friction to account creation
// it's better to impede sybil attacks by other means
#[near_bindgen]
impl FungibleTokenCore for MetaToken {
    #[payable]
    fn ft_transfer(&mut self, receiver_id: ValidAccountId, amount: U128, memo: Option<String>) {
        assert_one_yocto();
        let sender_id = env::predecessor_account_id();
        let amount: Balance = amount.into();
        self.internal_transfer(&sender_id, receiver_id.as_ref(), amount, memo);
    }

    #[payable]
    fn ft_transfer_call(
        &mut self,
        receiver_id: ValidAccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128> {
        assert_one_yocto();
        let sender_id = env::predecessor_account_id();
        let amount: Balance = amount.into();
        self.internal_transfer(&sender_id, receiver_id.as_ref(), amount, memo);
        // Initiating receiver's call and the callback
        // ext_fungible_token_receiver::ft_on_transfer(
        ext_ft_receiver::ft_on_transfer(
            sender_id.clone(),
            amount.into(),
            msg,
            receiver_id.as_ref(),
            NO_DEPOSIT,
            env::prepaid_gas() - GAS_FOR_FT_TRANSFER_CALL - GAS_FOR_RESOLVE_TRANSFER, // assign rest of gas to callback
        )
        .then(ext_self::ft_resolve_transfer(
            sender_id,
            receiver_id.into(),
            amount.into(),
            &env::current_account_id(),
            NO_DEPOSIT,
            GAS_FOR_RESOLVE_TRANSFER,
        ))
        .into()
    }

    fn ft_total_supply(&self) -> U128 {
        self.total_supply.into()
    }

    fn ft_balance_of(&self, account_id: ValidAccountId) -> U128 {
        self.accounts.get(account_id.as_ref()).unwrap_or(0).into()
    }
}

#[near_bindgen]
impl FungibleTokenResolver for MetaToken {
    /// Returns the amount of burned tokens in a corner case when the sender
    /// has deleted (unregistered) their account while the `ft_transfer_call` was still in flight.
    /// Returns (Used token amount, Burned token amount)
    #[private]
    fn ft_resolve_transfer(
        &mut self,
        sender_id: ValidAccountId,
        receiver_id: ValidAccountId,
        amount: U128,
    ) -> U128 {
        let sender_id: AccountId = sender_id.into();
        let (used_amount, burned_amount) =
            self.int_ft_resolve_transfer(&sender_id, receiver_id, amount);
        if burned_amount > 0 {
            log!("{} tokens burned", burned_amount);
        }
        return used_amount.into();
    }
}

#[near_bindgen]
impl FungibleTokenMetadataProvider for MetaToken {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        self.internal_get_ft_metadata()
    }
}

#[ext_contract(ext_ft_receiver)]
pub trait FungibleTokenReceiver {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128>;
}

#[ext_contract(ext_self)]
trait FungibleTokenResolver {
    fn ft_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        amount: U128,
    ) -> U128;
}
