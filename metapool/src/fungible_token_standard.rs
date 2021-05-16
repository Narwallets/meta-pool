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
        icon: Some(r#"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512"><path d="M340.9 114.4c0 8.4-10.1 15.5-22.7 21.1 -12.6 5.7-30.2 9-62.2 10V160l0 0v-14.4c-64-1.6-71.7-10.8-81.2-27.6l39.4-4.9c4.3 10.6 18.7 15.8 43.1 15.8 11.4 0 19.9-1.1 25.3-3.4 5.4-2.3 8.1-5 8.1-8.2 0-3.3-2.6-5.8-7.9-7.6 -5.4-1.7-17.1-3.8-35.7-6.5 -16.7-2.3-29.2-4.6-38.6-6.9 -9.4-2.2-16.1-5.4-21.9-9.5s-6.9-8.8-6.9-14.2c0-7.1 9-13.5 19.5-19.2C209.6 47.9 224 44.4 256 43.2V32l0 0v11.2c64 1.6 63.4 9.3 73.4 23l-34.1 6.8c-8.2-9.4-20.7-14.1-37.8-14.1 -8.6 0-15.4 1.1-20.6 3.2 -5.2 2.1-7.8 4.7-7.8 7.7 0 3 2.5 5.4 7.5 7 5 1.6 15.7 3.6 32.1 6.1 18 2.6 32.1 5.1 42.3 7.4 10.3 2.3 18.4 5.6 24.5 9.7C341.7 104.1 340.9 108.9 340.9 114.4zM448 128c0 11.3-4.1 22-11.2 32.1 0 0 0.1-0.1 0.1-0.1C443.9 170 448 180.8 448 192v32c0 11.3-4.1 22.1-11.2 32.1 0 0 0.1-0.1 0.1-0.1C443.9 266 448 276.8 448 288v32c0 11.3-4.1 22-11.2 32.1 0 0 0.1-0.1 0.1-0.1C443.9 362 448 372.8 448 384v32c0 53-86 96-192 96 -106 0-192-43-192-96v-32c0-11.2 4.1-22 11.2-32 0 0 0 0 0.1 0.1C68.1 342 64 331.3 64 320v-32c0-11.2 4.1-22 11.2-32 0 0 0 0.1 0.1 0.1C68.1 246 64 235.3 64 224v-32c0-11.2 4.1-22 11.2-32 0 0 0 0 0 0C68.1 150 64 139.3 64 128V96c0-53 86-96 192-96 106 0 192 43 192 96V128zM432 384c0-6.4-2.3-12.9-6.2-19.2 0.1-0.1 0.2-0.2 0.3-0.3C394 395.1 329.9 416 256 416c-73.6 0-137.4-20.7-169.7-51.1 0 0.1 0.1 0.1 0.1 0.2C82.7 371.3 80 377.7 80 384c0 37.8 72.3 80 176 80S432 421.8 432 384zM432 288c0-6.4-2.3-12.9-6.2-19.2 0.1-0.1 0.1-0.1 0.1-0.2C393.8 299.1 329.8 320 256 320c-73.6 0-137.4-20.7-169.7-51.1 0 0.1 0.1 0.1 0.1 0.2C82.7 275.3 80 281.7 80 288c0 37.8 72.3 80 176 80S432 325.8 432 288zM432 192c0-6.4-2.3-12.9-6.2-19.2 0-0.1 0-0.1 0.1-0.1C393.7 203.2 329.8 224 256 224c-73.5 0-137.3-20.7-169.6-51.1 0 0.1 0 0.1 0.1 0.1C82.7 179.3 80 185.7 80 192c0 37.8 72.3 80 176 80S432 229.8 432 192zM432 96c0-10.5-5.6-21.4-15.9-31.6C389.4 38 330.9 16 256 16 152.3 16 80 58.2 80 96s72.3 80 176 80S432 133.8 432 96z"/></svg>"#.into()),
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
