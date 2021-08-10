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
            r###"<svg viewBox="0 0 67.79 67.79"><ellipse style="fill:#fefefe;stroke-width:1.0235" cx="33.724" cy="33.996" rx="33.833" ry="33.841"/><ellipse style="fill:#a0a0a0;fill-opacity:1;stroke-width:.790979" cx="33.724" cy="33.996" rx="26.147" ry="26.153"/><path d="M7.884 51.716V49.65h.78q1.98 0 3.42-.541 1.5-.59 1.5-2.755V22.005q0-2.164-1.5-2.754-1.44-.59-3.42-.59h-.78v-2.067h13.68l12.42 28.137 12.301-28.137h13.32v2.066h-.78q-2.04 0-3.48.64-1.44.59-1.44 2.902v23.906q0 2.312 1.44 2.952 1.44.59 3.48.59h.78v2.066h-15.96V49.65h.18q1.92 0 3.06-.541 1.14-.541 1.26-2.558V20.284l-13.74 31.432h-3.24L17.243 20.382v25.726q0 2.312 1.14 2.952 1.14.59 3.18.59h.18v2.066z" aria-label="M" style="fill:#ffffff"/></svg>"###,
        )),
        reference: None, // TODO
        reference_hash: None,
        decimals: 24,
    }
}

impl MetaToken {
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

    pub fn internal_burn(&mut self, account_id: &AccountId, amount: u128) {
        let balance = self.internal_unwrap_balance_of(account_id);
        assert!(balance >= amount);
        self.internal_update_account(&account_id, balance - amount);
        assert!(self.total_supply >= amount);
        self.total_supply -= amount;
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

        if self.locked_until_nano > 0 && env::block_timestamp() < self.locked_until_nano {
            panic!(
                "transfers are locked until unix timestamp {}",
                self.locked_until_nano / NANOSECONDS
            );
        }

        let sender_balance = self.internal_unwrap_balance_of(sender_id);
        assert!(
            amount == sender_balance || amount > ONE_NEAR / MIN_TRANSFER_UNIT,
            "The amount should be at least 1/{}",
            MIN_TRANSFER_UNIT
        );

        // remove from sender
        let sender_balance = self.internal_unwrap_balance_of(sender_id);
        assert!(
            amount <= sender_balance,
            "The account doesn't have enough balance {}",
            sender_balance
        );
        let balance_left = sender_balance - amount;
        self.internal_update_account(&sender_id, balance_left);

        // check vesting
        if self.vested_count > 0 {
            match self.vested.get(&sender_id) {
                Some(vesting) => {
                    //compute locked
                    let locked = vesting.compute_amount_locked();
                    if locked == 0 {
                        //vesting is complete. remove vesting lock
                        self.vested.remove(&sender_id);
                        self.vested_count -= 1;
                    } else if balance_left < locked {
                        panic!("Vested account, balance can't go lower than {}", locked);
                    }
                }
                None => {}
            }
        }

        // add to receiver
        let receiver_balance = self.internal_unwrap_balance_of(receiver_id);
        self.internal_update_account(&receiver_id, receiver_balance + amount);

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
