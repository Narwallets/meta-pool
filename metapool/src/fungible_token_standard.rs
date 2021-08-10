use near_contract_standards::fungible_token::{
    core::FungibleTokenCore,
    metadata::{FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC},
};

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LazyOption;
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::{
    env, near_bindgen, AccountId, Balance, Gas, PanicOnDefault, PromiseOrValue, StorageUsage,
};

use crate::*;

/// Interface for callback after "on_multifuntok_transfer" to check if the receiving contract executed "on_multifuntok_transfer" ok
#[ext_contract(ext_self_callback)]
trait ExtMultiFunTokSelfCallback {
    //NEP-141 single fun token, for the default token
    fn after_ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        contract_id: AccountId,
        amount: U128String,
    );
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

const GAS_FOR_RESOLVE_TRANSFER: Gas = 5_000_000_000_000;
const GAS_FOR_FT_TRANSFER_CALL: Gas = 25_000_000_000_000 + GAS_FOR_RESOLVE_TRANSFER;
const NO_DEPOSIT: Balance = 0;

fn ft_metadata_default() -> FungibleTokenMetadata {
    FungibleTokenMetadata {
        spec: FT_METADATA_SPEC.to_string(),
        name: "Staked NEAR".to_string(),
        symbol: "STNEAR".to_string(),
        icon: Some(r#"<svg viewBox="0 0 67.79 67.79" version="1.1"><path style="fill:#fff" d="M33.934.311a33.9 33.9 0 1 0 33.89 33.9 33.9 33.9 0 0 0-33.89-33.9z" id="path505"/><path style="fill:#ffbd00;stroke:none;stroke-width:1px;stroke-linecap:butt;stroke-linejoin:miter;stroke-opacity:1;fill-opacity:1" d="m11.803 27.8 12.387.359 2.361 5.959 7.616 3.31 8.523-3.322 2.348-5.87 12.269.03L54.822 54.2 31.837 58.86 12.89 52.648z" id="path1051"/><path style="fill:#a0a0ff;stroke:none;stroke-width:1px;stroke-linecap:butt;stroke-linejoin:miter;stroke-opacity:1;fill-opacity:1" d="m34.657 12.575-10.43 9.633 1.096 10.01 8.844 5.21 9.785-5.287 1.086-11.33z" id="path1815"/><path id="path928" style="fill:#666;fill-opacity:1" d="M33.928 4.282a29.93 29.93 0 0 1 4.682.367 29.93 29.93 0 0 1 25.244 29.572 29.93 29.93 0 0 1-29.92 29.92 29.93 29.93 0 0 1-.006-59.86zm.729 8.293c-2.03 5.668-8.815 9.76-8.815 14.521 0 4.76 3.912 8.62 8.737 8.62 4.824 0 8.736-3.86 8.736-8.62s-6.707-8.697-8.658-14.521zM37.84 22.67a2.524 2.446 0 0 1 .246.012 2.524 2.446 0 0 1 .246.035 2.524 2.446 0 0 1 .24.059 2.524 2.446 0 0 1 .233.08 2.524 2.446 0 0 1 .225.104 2.524 2.446 0 0 1 .213.123 2.524 2.446 0 0 1 .197.142 2.524 2.446 0 0 1 .183.162 2.524 2.446 0 0 1 .168.178 2.524 2.446 0 0 1 .147.191 2.524 2.446 0 0 1 .127.207 2.524 2.446 0 0 1 .105.217 2.524 2.446 0 0 1 .084.227 2.524 2.446 0 0 1 .06.232 2.524 2.446 0 0 1 .038.237 2.524 2.446 0 0 1 .012.24 2.524 2.446 0 0 1-.086.633 2.524 2.446 0 0 1-.252.59 2.524 2.446 0 0 1-.403.507 2.524 2.446 0 0 1-.521.389 2.524 2.446 0 0 1-.61.244 2.524 2.446 0 0 1-.652.084 2.524 2.446 0 0 1-.654-.084 2.524 2.446 0 0 1-.607-.244 2.524 2.446 0 0 1-.524-.389 2.524 2.446 0 0 1-.4-.508 2.524 2.446 0 0 1-.252-.59 2.524 2.446 0 0 1-.086-.632 2.524 2.446 0 0 1 .086-.633 2.524 2.446 0 0 1 .252-.59 2.524 2.446 0 0 1 .4-.506A2.524 2.446 0 0 1 36.58 23a2.524 2.446 0 0 1 .607-.247 2.524 2.446 0 0 1 .654-.082zM24.19 28.16a16.579 2.485 0 0 0-6.502 1.965 16.579 2.485 0 0 0 7.635 2.093 10.483 10.6 0 0 1-1.133-4.058zm20.848.078a10.483 10.6 0 0 1-1.086 3.904 16.579 2.485 0 0 0 6.894-2.017 16.579 2.485 0 0 0-5.808-1.887zm6.925 3.21c-.455 1.177-4.097 2.154-9.273 2.659a10.483 10.6 0 0 1-8.072 3.861 10.483 10.6 0 0 1-8.067-3.85c-5.276-.506-8.978-1.498-9.398-2.64h-.049v5.17h.049a.69.69 0 0 0-.049.24c0 1.8 7.81 3.25 17.43 3.25 9.62 0 17.43-1.45 17.43-3.25a.69.69 0 0 0 0-.24zm.032 7.323c-.67 1.73-8.22 3.03-17.43 3.03-9.23 0-16.771-1.34-17.381-3h-.049v5.17h.049a.69.69 0 0 0-.049.24c0 1.8 7.81 3.25 17.43 3.25 9.62 0 17.43-1.45 17.43-3.25a.69.69 0 0 0 0-.24zm0 7.21c-.67 1.69-8.22 3.03-17.43 3.03-9.23 0-16.771-1.34-17.381-3h-.049v5.17h.049a.69.69 0 0 0-.049.24c0 1.8 7.81 3.25 17.43 3.25 9.62 0 17.43-1.45 17.43-3.25a.69.69 0 0 0 0-.24z"/></svg>"#.into()),
        reference: Some("https://narwallets.github.io/meta-pool".into()), // TODO
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
        receiver_id: ValidAccountId,
        amount: U128,
        #[allow(unused)] memo: Option<String>,
    ) {
        assert_one_yocto();
        self.internal_multifuntok_transfer(
            &env::predecessor_account_id(),
            &receiver_id.into(),
            STNEAR,
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

        self.assert_not_busy();
        self.contract_busy = true;

        let receiver: String = receiver_id.into();
        self.internal_multifuntok_transfer(
            &env::predecessor_account_id(),
            &receiver,
            STNEAR,
            amount.0,
        );

        //TODO add a busy lock to avoid the sender-acc to be deleted
        //while this txn itś executing
        //self.busy = true;

        ext_ft_receiver::ft_on_transfer(
            env::predecessor_account_id(),
            amount,
            msg,
            //promise params:
            &receiver,  //contract
            NO_DEPOSIT, //attached native NEAR amount
            env::prepaid_gas() - GAS_FOR_FT_TRANSFER_CALL,
        )
        .then(ext_self_callback::after_ft_on_transfer(
            env::predecessor_account_id(),
            receiver,
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
