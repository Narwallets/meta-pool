use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::{AccountId, Balance, PromiseResult};

use crate::*;

const ONE_NEAR: Balance = 1_000_000_000_000_000_000_000_000;
pub const MIN_TRANSFER_UNIT: u128 = 1000; // to make sibyl attacks more expensive in terms of tokens

pub fn default_ft_metadata() -> FungibleTokenMetadata {
    FungibleTokenMetadata {
        spec: FT_METADATA_SPEC.to_string(),
        name: "Meta Token".to_string(),
        symbol: "$META".to_string(),
        icon: Some(String::from(
            r###"<svg viewBox="0 0 468 325"><path transform="translate(0 0)scale(0.06 -0.07)" d="m2.4-155v-75h170 170v-925-924l-32-17c-25-13-67-19-170-22l-138-5v-73-74h425 425v75 75h-125c-127 0-176 10-203 42-9 12-12 203-10 904l3 889 34-70c18-38 73-158 121-265s176-388 285-625c109-236 251-551 316-700 125-281 152-325 205-325 49 0 81 27 113 95 43 91 586 1288 744 1640 71 160 131 292 133 294s4-419 4-937v-942h-170-170v-75-75h545 545v75 75h-175-175v945 945h175 175v75 76l-394-3c-452-3-418 4-466-103-15-33-176-388-358-790-414-914-393-868-396-864-2 2-97 211-211 464-414 917-547 1211-566 1241-10 17-30 36-44 42-18 9-137 12-405 12h-380v-75z"/></svg>"###,
        )),
        reference: None, // TODO
        reference_hash: None,
        decimals: 24,
    }
}

impl Contract {
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
