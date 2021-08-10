use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::{AccountId, Balance, PromiseResult};

use crate::*;

const ONE_NEAR: Balance = 1_000_000_000_000_000_000_000_000;
pub const MIN_TRANSFER_UNIT: u128 = 1000; // to make sibyl attacks more expensive in terms of tokens

pub fn default_ft_metadata() -> FungibleTokenMetadata {
    FungibleTokenMetadata {
        spec: FT_METADATA_SPEC.to_string(),
        name: "pNEAR token".to_string(),
        symbol: "$pNEAR".to_string(),
        icon: Some(r#"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512"><path d="M340.9 114.4c0 8.4-10.1 15.5-22.7 21.1 -12.6 5.7-30.2 9-62.2 10V160l0 0v-14.4c-64-1.6-71.7-10.8-81.2-27.6l39.4-4.9c4.3 10.6 18.7 15.8 43.1 15.8 11.4 0 19.9-1.1 25.3-3.4 5.4-2.3 8.1-5 8.1-8.2 0-3.3-2.6-5.8-7.9-7.6 -5.4-1.7-17.1-3.8-35.7-6.5 -16.7-2.3-29.2-4.6-38.6-6.9 -9.4-2.2-16.1-5.4-21.9-9.5s-6.9-8.8-6.9-14.2c0-7.1 9-13.5 19.5-19.2C209.6 47.9 224 44.4 256 43.2V32l0 0v11.2c64 1.6 63.4 9.3 73.4 23l-34.1 6.8c-8.2-9.4-20.7-14.1-37.8-14.1 -8.6 0-15.4 1.1-20.6 3.2 -5.2 2.1-7.8 4.7-7.8 7.7 0 3 2.5 5.4 7.5 7 5 1.6 15.7 3.6 32.1 6.1 18 2.6 32.1 5.1 42.3 7.4 10.3 2.3 18.4 5.6 24.5 9.7C341.7 104.1 340.9 108.9 340.9 114.4zM448 128c0 11.3-4.1 22-11.2 32.1 0 0 0.1-0.1 0.1-0.1C443.9 170 448 180.8 448 192v32c0 11.3-4.1 22.1-11.2 32.1 0 0 0.1-0.1 0.1-0.1C443.9 266 448 276.8 448 288v32c0 11.3-4.1 22-11.2 32.1 0 0 0.1-0.1 0.1-0.1C443.9 362 448 372.8 448 384v32c0 53-86 96-192 96 -106 0-192-43-192-96v-32c0-11.2 4.1-22 11.2-32 0 0 0 0 0.1 0.1C68.1 342 64 331.3 64 320v-32c0-11.2 4.1-22 11.2-32 0 0 0 0.1 0.1 0.1C68.1 246 64 235.3 64 224v-32c0-11.2 4.1-22 11.2-32 0 0 0 0 0 0C68.1 150 64 139.3 64 128V96c0-53 86-96 192-96 106 0 192 43 192 96V128zM432 384c0-6.4-2.3-12.9-6.2-19.2 0.1-0.1 0.2-0.2 0.3-0.3C394 395.1 329.9 416 256 416c-73.6 0-137.4-20.7-169.7-51.1 0 0.1 0.1 0.1 0.1 0.2C82.7 371.3 80 377.7 80 384c0 37.8 72.3 80 176 80S432 421.8 432 384zM432 288c0-6.4-2.3-12.9-6.2-19.2 0.1-0.1 0.1-0.1 0.1-0.2C393.8 299.1 329.8 320 256 320c-73.6 0-137.4-20.7-169.7-51.1 0 0.1 0.1 0.1 0.1 0.2C82.7 275.3 80 281.7 80 288c0 37.8 72.3 80 176 80S432 325.8 432 288zM432 192c0-6.4-2.3-12.9-6.2-19.2 0-0.1 0-0.1 0.1-0.1C393.7 203.2 329.8 224 256 224c-73.5 0-137.3-20.7-169.6-51.1 0 0.1 0 0.1 0.1 0.1C82.7 179.3 80 185.7 80 192c0 37.8 72.3 80 176 80S432 229.8 432 192zM432 96c0-10.5-5.6-21.4-15.9-31.6C389.4 38 330.9 16 256 16 152.3 16 80 58.2 80 96s72.3 80 176 80S432 133.8 432 96z"/></svg>"#.into()),
        reference: None, // TODO
        reference_hash: None,
        decimals: 24,
    }
}

impl Contract {


    pub fn assert_can_operate(&self) {
        assert!(self.can_operate(), "operation is not open");
    }
    
    pub fn assert_owner_calling(&self) {
        assert!(
            env::predecessor_account_id() == self.owner_id,
            "can only be called by the owner"
        );
    }

    pub fn assert_minter(&self, account_id: String) {
        assert!(self.minters.contains(&account_id), "not a minter");
    }

    //get stored metadata or default
    pub fn internal_get_ft_metadata(&self) -> FungibleTokenMetadata {
        self.metadata.get().unwrap_or(default_ft_metadata())
    }

    pub fn internal_unwrap_balance_of(&self, account_id: &AccountId) -> Balance {
        match self.accounts.get(&account_id) {
            Some(balance) => balance,
            // Q: This makes the contract vulnerable to the sybil attack on storage.
            // Since `ft_transfer` is cheaper than storage for 1 account, you can send
            // 1 token to a ton randomly generated accounts and it will require 125 bytes per
            // such account. So it would require 800 transactions to block 1 NEAR of the account.
            // R: making the user manage storage costs adds too much friction to account creation
            // it's better to impede sybil attacks by other means
            // there's a MIN_TRANSFER of 1/1000 to make sibyl attacks more expensive in terms of tokens
            None => 0,
        }
    }

    pub fn mint_into(&mut self, account_id: &AccountId, amount: Balance) {
        let balance = self.internal_unwrap_balance_of(account_id);
        self.internal_update_account(&account_id, balance + amount);
        self.total_supply += amount;
    }

    pub fn internal_transfer(
        &mut self,
        sender_id: &AccountId,
        receiver_id: &AccountId,
        amount: Balance,
        memo: Option<String>,
    ) {
        assert_ne!(
            sender_id, receiver_id,
            "Sender and receiver should be different"
        );
        let sender_balance = self.internal_unwrap_balance_of(sender_id);
        assert!(
            amount == sender_balance || amount > ONE_NEAR / MIN_TRANSFER_UNIT,
            "The amount should be at least 1/{}",
            MIN_TRANSFER_UNIT
        );
        // remove from sender
        {
            assert!(
                amount <= sender_balance,
                "The account doesn't have enough balance {}",
                sender_balance
            );
            self.internal_update_account(&sender_id, sender_balance - amount);
        }
        // add to receiver
        {
            let receiver_balance = self.internal_unwrap_balance_of(receiver_id);
            self.internal_update_account(&receiver_id, receiver_balance + amount);
        }
        log!("Transfer {} from {} to {}", amount, sender_id, receiver_id);
        if let Some(memo) = memo {
            log!("Memo: {}", memo);
        }
    }

    /// Inner method to save the given account for a given account ID.
    /// If the account balance is 0, the account is deleted instead to release storage.
    pub fn internal_update_account(&mut self, account_id: &AccountId, balance: u128) {
        if balance == 0 {
            self.accounts.remove(account_id);
        } else {
            self.accounts.insert(account_id, &balance); //insert_or_update
        }
    }

    // TODO rename
    pub fn int_ft_resolve_transfer(
        &mut self,
        sender_id: &AccountId,
        receiver_id: ValidAccountId,
        amount: U128,
    ) -> (u128, u128) {
        let sender_id: AccountId = sender_id.into();
        let receiver_id: AccountId = receiver_id.into();
        let amount: Balance = amount.into();

        // Get the unused amount from the `ft_on_transfer` call result.
        let unused_amount = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(value) => {
                if let Ok(unused_amount) = near_sdk::serde_json::from_slice::<U128>(&value) {
                    std::cmp::min(amount, unused_amount.0)
                } else {
                    amount
                }
            }
            PromiseResult::Failed => amount,
        };

        if unused_amount > 0 {
            let receiver_balance = self.accounts.get(&receiver_id).unwrap_or(0);
            if receiver_balance > 0 {
                let refund_amount = std::cmp::min(receiver_balance, unused_amount);
                self.accounts
                    .insert(&receiver_id, &(receiver_balance - refund_amount));

                if let Some(sender_balance) = self.accounts.get(&sender_id) {
                    self.accounts
                        .insert(&sender_id, &(sender_balance + refund_amount));
                    log!(
                        "Refund {} from {} to {}",
                        refund_amount,
                        receiver_id,
                        sender_id
                    );
                    return (amount - refund_amount, 0);
                } else {
                    // Sender's account was deleted, so we need to burn tokens.
                    self.total_supply -= refund_amount;
                    log!("The account of the sender was deleted");
                    return (amount, refund_amount);
                }
            }
        }
        (amount, 0)
    }
}
