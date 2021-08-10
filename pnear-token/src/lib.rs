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
    assert_one_yocto, is_promise_success, env, ext_contract, log, near_bindgen, AccountId, Balance, Gas,
    PanicOnDefault, Promise,PromiseOrValue,EpochHeight,
};

mod util;
use crate::util::proportional;

const ONE_NEAR: Balance = 1_000_000_000_000_000_000_000_000;
const NEAR: Balance = ONE_NEAR;
const TGAS: Gas = 1_000_000_000_000;
const GAS_FOR_RESOLVE_TRANSFER: Gas = 5 * TGAS;
const GAS_FOR_FT_TRANSFER_CALL: Gas = 25 * TGAS + GAS_FOR_RESOLVE_TRANSFER;

// FIXED: You need 20 TGAS for 2 function calls and 1 .then
const GAS_FOR_OPEN: Gas = 10 * TGAS + 20 * TGAS;
const GAS_FOR_BUY: Gas = 25 * TGAS;
const GAS_FOR_AFTER_FT_BALANCE: Gas = 10 * TGAS;
const GAS_FOR_AFTER_BUY: Gas = 5 * TGAS;

const NO_DEPOSIT: Balance = 0;

type U128String = U128;
/// Interface for callback after "on_multifuntok_transfer" to check if the receiving contract executed "on_multifuntok_transfer" ok
#[ext_contract(ext_self_callback)]
pub trait ExtContract {
    fn after_ft_balance(&mut self);
    fn after_buy(
        &mut self,
        user_account: AccountId,
        near_amount: U128String,
        token_amount: U128String,
    );
    fn after_sell(&mut self, near_amount: U128String, token_amount: U128String);
}

near_sdk::setup_alloc!();

mod internal;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    metadata: LazyOption<FungibleTokenMetadata>,


    pub token_contract: String,

    pub sell_only: bool,
    pub min_amount_stnear: u128,
    pub min_amount_token: u128,
    pub tokens_left: u128,

    pub accounts: LookupMap<AccountId, Balance>,

    pub owner_id: AccountId,

    pub minters: Vec<AccountId>,

    pub total_supply: Balance,

    pub is_open: bool,
    pub near_received: u128,
}

#[near_bindgen]
impl Contract {
    /// Initializes the contract with the given total supply owned by the given `owner_id`.

    #[init]
    pub fn new(
        owner_id: AccountId,
        token_contract: String,
        sell_only: bool,
    ) -> Self {


        Self {
            owner_id: owner_id.clone(),
            token_contract,
            min_amount_stnear: 5,
            min_amount_token: 0,
            sell_only,
            metadata: LazyOption::new(b"m".to_vec(), None),
            accounts: LookupMap::new(b"a".to_vec()),
            minters: vec![owner_id],
            total_supply: 0,
            is_open: true,
            tokens_left: 0,
            near_received: 0,
        }
    }

    /// Returns account ID of the owner.
    pub fn get_owner_id(&self) -> AccountId {
        return self.owner_id.clone();
    }
    pub fn set_owner_id(&mut self, owner_id: AccountId) {
        self.assert_owner_calling();
        self.owner_id = owner_id.into();
    }

    //owner can mint more into their account
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

    /// Returns account ID of the staking pool owner.
    #[payable]
    pub fn set_metadata_icon(&mut self, svg_string: String) {
        assert_one_yocto();
        self.assert_owner_calling();
        let mut m = self.internal_get_ft_metadata();
        m.icon = Some(svg_string);
        self.metadata.set(&m);
    }

    /// Returns account ID of the staking pool owner.
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


    //-------------------//-------------------
    // SELL - user calls ft_transfer_call to send us tokens
    // and the ft_transfer_call from the token calls here
    // ft_on_transfer(  sender_id.clone(),  amount.into(),  msg ) -> PromiseOrValue<U128>
    //-------------------//-------------------
    #[allow(unused_variables)]
    pub fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128String,
        // Note: `msg` is expected by the standard, so `_msg` will not be found and will generate a
        //    deserialization error. That's why the `#[allow(unused_variables)]` on the
        //    method. But since JSON parsing is permissive, it's also possible to drop `msg`
        //    completely from the list of arguments.
        msg: String,
    ) -> PromiseOrValue<U128> {
        self.assert_can_operate();
        assert!(!self.sell_only, "You can only buy on this pool");
        //this fn should only be called by the token contract
        assert_eq!(env::predecessor_account_id(), self.token_contract);

        let token_amount = amount.0;

        assert!(
            token_amount >= self.min_amount_stnear,
            "min amount is {} tokens",
            self.min_amount_stnear
        );
        //compute near amount with price computed *after* we buy the tokens
        let near_amount = token_amount;

        // Similar to `buy` method, we update `near_received` and `tokens_left` now, and rollback in case of error
        self.near_received -= near_amount;
        self.tokens_left += token_amount;

        //transfer NEAR to the user
        Promise::new(sender_id)
            .transfer(near_amount)
            .then(ext_self_callback::after_sell(
                near_amount.into(),
                token_amount.into(),
                //promise params:
                &env::current_account_id(), //callback
                NO_DEPOSIT,                 //attached native NEAR amount
                GAS_FOR_AFTER_BUY,
            ))
            .into()
    }
    //prev fn continues here
    #[private]
    pub fn after_sell(&mut self, near_amount: U128String, token_amount: U128String) -> U128String {
        if is_promise_success() {
            // transfer was ok
            return 0.into(); //we used all user's the tokens
        } else {
            // NEAR transfer failed
            // rollback changes
            self.near_received += near_amount.0;
            self.tokens_left -= token_amount.0;
            return token_amount.into(); //return tokens to user
        }
    }

    pub fn can_operate(&self) -> bool {
        if !self.is_open {
            return false;
        }
        return true;
    }
}

// Q: Is ignoring storage costs the only reason for the re-implementation?
// making the user manage storage costs adds too much friction to account creation
// it's better to impede sybil attacks by other means
#[near_bindgen]
impl FungibleTokenCore for Contract {
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
            env::prepaid_gas() - GAS_FOR_FT_TRANSFER_CALL,
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
impl FungibleTokenResolver for Contract {
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
impl FungibleTokenMetadataProvider for Contract {
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

/*
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, Balance};

    use super::*;

    const OWNER_SUPPLY: Balance = 1_000_000_000_000_000_000_000_000_000_000;

    fn get_context(predecessor_account_id: ValidAccountId) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(accounts(0))
            .signer_account_id(predecessor_account_id.clone())
            .predecessor_account_id(predecessor_account_id);
        builder
    }

    #[test]
    fn test_new() {
        let mut context = get_context(accounts(1));
        testing_env!(context.build());
        let mut contract = Contract::new(accounts(1).into());
        contract.mint(&accounts(1).to_string(), OWNER_SUPPLY.into());
        testing_env!(context.is_view(true).build());
        assert_eq!(contract.ft_total_supply().0, OWNER_SUPPLY);
        assert_eq!(contract.ft_balance_of(accounts(1)).0, OWNER_SUPPLY);
    }

    #[test]
    #[should_panic(expected = "The contract is not initialized")]
    fn test_default() {
        let context = get_context(accounts(1));
        testing_env!(context.build());
        let _contract = Contract::default();
    }

    #[test]
    fn test_transfer() {
        let mut context = get_context(accounts(2));
        testing_env!(context.build());
        let mut contract = Contract::new(accounts(2).into());
        contract.mint(&accounts(2).to_string(), OWNER_SUPPLY.into());
        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(1_000_000_000_000_000)
            .predecessor_account_id(accounts(1))
            .build());

        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(1)
            .predecessor_account_id(accounts(2))
            .build());
        let transfer_amount = OWNER_SUPPLY / 3;
        contract.ft_transfer(accounts(1), transfer_amount.into(), None);

        testing_env!(context
            .storage_usage(env::storage_usage())
            .account_balance(env::account_balance())
            .is_view(true)
            .attached_deposit(0)
            .build());
        assert_eq!(
            contract.ft_balance_of(accounts(2)).0,
            (OWNER_SUPPLY - transfer_amount)
        );
        assert_eq!(contract.ft_balance_of(accounts(1)).0, transfer_amount);
    }
}
*/
