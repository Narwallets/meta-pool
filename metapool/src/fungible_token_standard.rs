use near_contract_standards::fungible_token::{
    core::FungibleTokenCore,
    metadata::{FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC},
    resolver::FungibleTokenResolver,
};

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LazyOption;
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::{
    env, near_bindgen, AccountId, Balance, Gas, PanicOnDefault, PromiseOrValue, StorageUsage,
};

use crate::*;

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

const GAS_FOR_FT_TRANSFER_CALL: Gas = 30_000_000_000_000;
const GAS_FOR_RESOLVE_TRANSFER: Gas = 11_000_000_000_000;
const FIVE_TGAS: Gas = 5_000_000_000_000;
const ONE_TGAS: Gas = 1_000_000_000_000;

const NO_DEPOSIT: Balance = 0;

fn ft_metadata_default() -> FungibleTokenMetadata {
    FungibleTokenMetadata {
        spec: FT_METADATA_SPEC.to_string(),
        name: "Staked NEAR".to_string(),
        symbol: "STNEAR".to_string(),
        icon: Some(r#"data:image/svg+xml,%3csvg width='96' height='96' viewBox='0 0 96 96' fill='none' xmlns='http://www.w3.org/2000/svg'%3e%3crect width='96' height='96' rx='48' fill='white'/%3e%3cpath fill-rule='evenodd' clip-rule='evenodd' d='M48.0006 20L41.2575 26.7431L48.0006 33.4862L54.7437 26.7431L48.0006 20ZM37.281 30.7188L30.7144 37.2853L47.9998 54.5707L65.2851 37.2853L58.7186 30.7188L47.9998 41.4376L37.281 30.7188ZM26.7384 41.261L19.9953 48.0041L47.9995 76.0083L76.0037 48.0041L69.2606 41.2611L47.9995 62.5221L26.7384 41.261Z' fill='%23231B51'/%3e%3c/svg%3e"#.into()),
        reference: Some("https://metapool.app".into()), 
        reference_hash: None,
        decimals: 24,
    }
}
fn ft_metadata_init_lazy_container() -> LazyOption<FungibleTokenMetadata> {
    let metadata: LazyOption<FungibleTokenMetadata>;
    metadata = LazyOption::new(b"ftmd".to_vec(), None);
    return metadata;
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    metadata: LazyOption<FungibleTokenMetadata>,

    pub accounts: LookupMap<AccountId, Balance>,
    pub total_supply: Balance,
    // TODO: rename
    /// The storage size in bytes for one account.
    pub account_storage_usage: StorageUsage,
}

#[near_bindgen]
impl FungibleTokenCore for MetaPool {
    //NEP-141 for default token STNEAR, ft_transfer
    /// Transfer `amount` of tokens from the caller of the contract (`predecessor_id`) to a contract at `receiver_id`.
    /// Requirements:
    /// * receiver_id must be a contract and must respond to `ft_on_transfer(&mut self, sender_id: AccountId, amount: U128String, _msg: String ) -> PromiseOrValue<U128>`
    /// * if receiver_id is not a contract or `ft_on_transfer` fails, the transfer is rolled-back
    #[payable]
    fn ft_transfer(
        &mut self,
        receiver_id: ValidAccountId, // ValidAccountId does not adds gas consumption
        amount: U128,
        #[allow(unused)] memo: Option<String>,
    ) {
        assert_one_yocto();
        self.internal_st_near_transfer(
            &env::predecessor_account_id(),
            &receiver_id.into(),
            amount.0,
        );
    }

    #[payable]
    fn ft_transfer_call(
        &mut self,
        receiver_id: ValidAccountId,
        amount: U128,
        #[allow(unused)] memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128> {
        assert_one_yocto();
        assert!(
            env::prepaid_gas() > GAS_FOR_FT_TRANSFER_CALL + GAS_FOR_RESOLVE_TRANSFER + FIVE_TGAS,
            "gas required {}",
            GAS_FOR_FT_TRANSFER_CALL + GAS_FOR_RESOLVE_TRANSFER + FIVE_TGAS
        );

        let receiver_id: String = receiver_id.into();
        self.internal_st_near_transfer(&env::predecessor_account_id(), &receiver_id, amount.0);

        //TODO add a busy lock to avoid the sender-acc to be deleted
        //while this txn is executing
        //self.busy = true;

        ext_ft_receiver::ft_on_transfer(
            env::predecessor_account_id(),
            amount,
            msg,
            //promise params:
            &receiver_id, //contract
            NO_DEPOSIT,   //attached native NEAR amount
            env::prepaid_gas() - GAS_FOR_FT_TRANSFER_CALL - GAS_FOR_RESOLVE_TRANSFER - ONE_TGAS, // set almost all remaining gas for ft_on_transfer
        )
        .then(ext_self::ft_resolve_transfer(
            env::predecessor_account_id(),
            receiver_id,
            amount,
            //promise params:
            &env::current_account_id(), //contract
            NO_DEPOSIT,                 //attached native NEAR amount
            GAS_FOR_RESOLVE_TRANSFER,
        ))
        .into()
    }

    //stNEAR total supply
    fn ft_total_supply(&self) -> U128 {
        self.total_stake_shares.into()
    }

    fn ft_balance_of(&self, account_id: ValidAccountId) -> U128 {
        let acc = self.internal_get_account(&account_id.into());
        return acc.stake_shares.into();
    }
}

#[near_bindgen]
impl FungibleTokenResolver for MetaPool {
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
impl FungibleTokenMetadataProvider for MetaPool {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        let metadata = ft_metadata_init_lazy_container();
        //load from storage or return default
        return metadata.get().unwrap_or(ft_metadata_default());
    }
}

#[near_bindgen]
impl MetaPool {
    pub fn ft_metadata_set(&self, data: FungibleTokenMetadata) {
        let mut metadata = ft_metadata_init_lazy_container();
        metadata.set(&data); //save into storage
    }
}
