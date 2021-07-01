#![allow(unused_imports)]
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{ValidAccountId, U128, U64};
use near_sdk::{
    assert_one_yocto, env, ext_contract, is_promise_success, log, near_bindgen, AccountId, Balance,
    EpochHeight, Gas, PanicOnDefault, Promise, PromiseOrValue,
};

mod util;
use crate::util::proportional;

const ONE_NEAR: Balance = 1_000_000_000_000_000_000_000_000;
const NEAR: Balance = ONE_NEAR;
const TGAS: Gas = 1_000_000_000_000;

// FIXED: You need 20 TGAS for 2 function calls and 1 .then
const GAS_FOR_OPEN: Gas = 10 * TGAS + 20 * TGAS;
const GAS_FOR_BUY: Gas = 25 * TGAS;
const GAS_FOR_AFTER_FT_BALANCE: Gas = 10 * TGAS;
const GAS_FOR_AFTER_BUY: Gas = 5 * TGAS;

const NO_DEPOSIT: Balance = 0;
const ONE_YOCTO: Balance = 1;

type U128String = U128;

#[ext_contract(ext_ft_contract)]
pub trait FungibleToken {
    fn ft_balance_of(&mut self, account_id: AccountId) -> U128String;
    fn ft_transfer(
        &mut self,
        user_account: AccountId,
        token_amount: U128String,
        msg: Option<String>,
    );
}

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
    pub owner_id: AccountId,

    pub total_near: u128,
    pub total_tokens: u128,

    pub token_contract: String,

    ///initial price * 1e3, e.g. 1025 => price = 1.025
    pub initial_price_e3: u128,
    ///final price * 1e3, e.g. 1225 => price = 1.225
    pub final_price_e3: u128,

    pub sell_only: bool,
    pub min_amount_near: u128,
    pub min_amount_token: u128,

    pub opens_at_epoch: EpochHeight,
    pub closes_at_epoch: EpochHeight,

    pub is_open: bool,
    pub tokens_left: u128,
    pub near_received: u128,
}

#[near_bindgen]
impl Contract {
    /// Initializes the contract with the given total supply owned by the given `owner_id`.

    #[init]
    pub fn new(
        owner_id: AccountId,
        total_near: U128String,
        total_tokens: U128String,
        token_contract: String,
        initial_price_e3: U128String,
        final_price_e3: U128String,
        opens_at_epoch: EpochHeight,
        closes_at_epoch: EpochHeight,
        min_amount_near: U128String,
        min_amount_token: U128String,
        sell_only: bool,
    ) -> Self {
        assert!(
            initial_price_e3.0 <= final_price_e3.0,
            "final price must be >= initial price"
        );
        assert!(
            closes_at_epoch == 0 || opens_at_epoch == 0 || closes_at_epoch > opens_at_epoch,
            "closes_at_epoch must be >= opens_at_epoch"
        );

        assert!(total_tokens.0 > 1 * NEAR);
        assert!(total_near.0 > 1 * NEAR);

        Self {
            owner_id: owner_id.clone(),
            total_near: total_near.into(),
            total_tokens: total_tokens.into(),
            token_contract,
            initial_price_e3: initial_price_e3.into(),
            final_price_e3: final_price_e3.into(),
            opens_at_epoch: opens_at_epoch.into(),
            closes_at_epoch: closes_at_epoch.into(),
            min_amount_near: min_amount_near.into(),
            min_amount_token: min_amount_token.into(),
            sell_only,

            is_open: false,
            tokens_left: 0,
            near_received: 0,
        }
    }

    /// Returns account ID of the owner.
    pub fn get_owner_id(&self) -> AccountId {
        return self.owner_id.clone();
    }

    /// Open
    pub fn open(&self) -> Promise {
        self.assert_owner_calling();
        assert!(!self.is_open);
        if !self.sell_only {
            //check that we have enough NEAR
            assert!(
                env::account_balance() >= self.total_near + ONE_NEAR,
                "sell/buy mode, not enough NEAR balance"
            );
        }
        //schedule promise: ask how much tokens we have
        ext_ft_contract::ft_balance_of(
            env::current_account_id(), //this contract
            //promise params:
            &self.token_contract, //call token contract
            NO_DEPOSIT,           //attached native NEAR amount
            env::prepaid_gas() - GAS_FOR_OPEN - GAS_FOR_AFTER_FT_BALANCE,
        )
        .then(ext_self_callback::after_ft_balance(
            //promise params:
            &env::current_account_id(), //callback
            NO_DEPOSIT,                 //attached native NEAR amount
            GAS_FOR_AFTER_FT_BALANCE,
        ))
    }
    //prev fn continues here
    #[private]
    pub fn after_ft_balance(&mut self, #[callback] token_balance: U128String) {
        //check that we have enough tokens
        // NOTE: is_promise_success() is not needed, because `#[callback]` fails at deserialization
        //     for failed promises
        assert!(
            // Fixed
            token_balance.0 == self.total_tokens,
            "Incorrect tokens at account {}: {}. Expected exactly {}",
            env::current_account_id(),
            self.token_contract,
            self.total_tokens
        );

        self.tokens_left = token_balance.0;

        self.is_open = true;
    }

    pub fn can_operate(&self) -> bool {
        if !self.is_open {
            return false;
        }
        if self.opens_at_epoch != 0 && env::epoch_height() < self.opens_at_epoch {
            return false;
        }
        if self.closes_at_epoch != 0 && env::epoch_height() >= self.closes_at_epoch {
            return false;
        }
        return true;
    }

    //-------------------
    //buy
    //-------------------
    #[payable]
    pub fn buy(&mut self) -> Promise {
        self.assert_can_operate();

        let user_account = env::predecessor_account_id();
        let attached_near = env::attached_deposit();

        assert!(
            attached_near >= self.min_amount_near,
            "min amount is {}",
            self.min_amount_near
        );
        assert!(
            attached_near <= self.total_near - self.near_received,
            "max amount is {} NEAR",
            self.total_near - self.near_received
        );
        //compute token amount with price computed *after* we sell them (buying all pays the higher price)
        let token_amount = if self.initial_price_e3 == self.final_price_e3 {
            //easy mode
            proportional(attached_near, 1000, self.initial_price_e3)
        } else {
            // NOTE: This seems a bit unfair and makes buying in small batches a better price,
            // but it is normal uniswap-like price calculation for operations that affect a large portion of the pool
            let delta_price = self.final_price_e3 - self.initial_price_e3;
            let near_after = self.near_received + attached_near;
            let price_after_e3 =
                self.initial_price_e3 + proportional(delta_price, near_after, self.total_near);
            proportional(attached_near, 1000, price_after_e3)
        };

        // This methods update `near_received` and `tokens_left`, so it's not possible
        //    to call it multiple times and buy at low price multiple times.
        //    `near_received` and `tokens_left` are updated immediately and rolled back
        //    in case `ft_transfer` fails.
        self.near_received += attached_near;
        self.tokens_left -= token_amount;

        //transfer tokens to user
        ext_ft_contract::ft_transfer(
            user_account.clone(),
            token_amount.into(),
            None,
            //promise params:
            &self.token_contract, //call token contract
            ONE_YOCTO,            //attached native NEAR amount
            env::prepaid_gas() - GAS_FOR_BUY - GAS_FOR_AFTER_BUY,
        )
        .then(ext_self_callback::after_buy(
            user_account,
            attached_near.into(),
            token_amount.into(),
            //promise params:
            &env::current_account_id(), //callback
            NO_DEPOSIT,                 //attached native NEAR amount
            GAS_FOR_AFTER_BUY,
        ))
    }
    //prev fn continues here
    #[private]
    pub fn after_buy(
        &mut self,
        user_account: AccountId,
        near_amount: U128String,
        token_amount: U128String,
    ) -> U128String {
        if is_promise_success() {
            //transfer was ok
            return token_amount.into();
        } else {
            // ft_transfer failed
            // rollback changes
            self.near_received -= near_amount.0;
            self.tokens_left += token_amount.0;
            // return NEAR to the user
            // NOTE: Subscribing ONE_YOCTO spends more gas and generates more in 30% gas reward,
            //    than not doing it. So it's better just to ignore this ONE_YOCTO.
            Promise::new(user_account).transfer(near_amount.0 - ONE_YOCTO);
            return 0.into();
        }
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
            token_amount >= self.min_amount_token,
            "min amount is {} tokens",
            self.min_amount_token
        );
        assert!(
            token_amount <= self.tokens_left,
            "max amount is {} tokens",
            self.tokens_left
        );
        //compute near amount with price computed *after* we buy the tokens
        let near_amount = if self.initial_price_e3 == self.final_price_e3 {
            //easy mode
            proportional(token_amount, self.initial_price_e3, 1000)
        } else {
            let delta_price = self.final_price_e3 - self.initial_price_e3;
            let token_after = self.tokens_left - token_amount;
            let price_after_e3 =
                self.initial_price_e3 + proportional(delta_price, token_after, self.total_tokens);
            proportional(token_amount, price_after_e3, 1000)
        };

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
}

/*
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, Balance};

    use super::*;

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
        //let mut context = get_context(accounts(1));
        //testing_env!(context.build());
        // let mut contract = Contract::new(accounts(1).into());
        // contract.mint(&accounts(1).to_string(), OWNER_SUPPLY.into());
        // testing_env!(context.is_view(true).build());
        // assert_eq!(contract.ft_total_supply().0, OWNER_SUPPLY);
        // assert_eq!(contract.ft_balance_of(accounts(1)).0, OWNER_SUPPLY);
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
        // let mut contract = Contract::new(accounts(2).into());
        // contract.mint(&accounts(2).to_string(), OWNER_SUPPLY.into());
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
        // let transfer_amount = OWNER_SUPPLY / 3;
        // contract.ft_transfer(accounts(1), transfer_amount.into(), None);

        testing_env!(context
            .storage_usage(env::storage_usage())
            .account_balance(env::account_balance())
            .is_view(true)
            .attached_deposit(0)
            .build());
        // assert_eq!(
        //     contract.ft_balance_of(accounts(2)).0,
        //     (OWNER_SUPPLY - transfer_amount)
        // );
        // assert_eq!(contract.ft_balance_of(accounts(1)).0, transfer_amount);
    }
}
*/
