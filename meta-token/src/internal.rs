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
            r#"data:image/svg+xml,%3csvg width='96' height='96' viewBox='0 0 96 96' fill='none' xmlns='http://www.w3.org/2000/svg'%3e%3crect width='96' height='96' rx='48' fill='white'/%3e%3cpath d='M29.2241 28.7456C28.396 27.9423 27.0094 28.5289 27.0091 29.6825L27 66.6773C26.9997 67.8501 28.4257 68.4286 29.2426 67.5872L48.6529 47.5943L29.2241 28.7456Z' fill='%23231B51'/%3e%3cpath d='M66.7759 28.7456C67.604 27.9423 68.9906 28.5289 68.9909 29.6825L69 66.6773C69.0003 67.8501 67.5743 68.4286 66.7574 67.5872L47.3471 47.5943L66.7759 28.7456Z' fill='%23231B51'/%3e%3c/svg%3e"#,
        )),
        reference: Some("https://metapool.app".into()),
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
        self.accounts.get(&account_id).unwrap_or(0)
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
                        panic!("Vested account, balance can not go lower than {}", locked);
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
    pub fn internal_update_account(&mut self, account_id: &AccountId, balance: u128) {
        self.accounts.insert(account_id, &balance); //insert_or_update
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
