use near_sdk::near_bindgen;
use crate::*;

#[near_bindgen]
impl DiversifiedPool {

    /// Returns the account ID of the owner.
    pub fn get_owner_account_id(&self) -> AccountId {
        self.owner_account_id.clone()
    }

    /// The amount of tokens that were deposited to the staking pool.
    /// NOTE: The actual balance can be larger than this known deposit balance due to staking
    /// rewards acquired on the staking pool.
    /// To refresh the amount the owner can call `refresh_staking_pool_balance`.
    pub fn get_known_deposited_balance(&self) -> U128String {
        return self.total_actually_staked.into()
    }

    /// full account info
    /// Returns JSON representation of the account for the given account ID.
    pub fn get_account_info(&self, account_id: AccountId) -> GetAccountInfoResult {
        let account = self.internal_get_account(&account_id);
        let staked_plus_rewards = self.amount_from_shares(account.stake_shares);
        let current_rewards = staked_plus_rewards.saturating_sub(account.staked);
        return GetAccountInfoResult {
            account_id,
            available: account.available.into(),
            skash: account.staked.into(),
            rewards: current_rewards.into(),
            historic_rewards: (current_rewards + account.accumulated_withdrew_rewards).into(),
            unstaked: account.unstaked.into(),
            unstaked_requested_epoch_height: account.unstaked_requested_epoch_height.into(),
            can_withdraw: (env::epoch_height()>=account.unstaked_requested_epoch_height+NUM_EPOCHS_TO_UNLOCK),
            total: (account.available + account.staked + current_rewards + account.unstaked).into()
        }
    }



}

