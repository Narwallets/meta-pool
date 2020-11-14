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


    //pub fn serialized result
    pub fn get_account_info(&self, a:AccountId) -> AccountInfoResult {
        let acc = self.internal_get_account(&a);
        let staked_plus_benef = self.amount_from_shares(acc.stake_shares);
        let benefits = staked_plus_benef.saturating_sub(acc.staked);
        return AccountInfoResult {
            total: (acc.available + staked_plus_benef + acc.unstaked).into(),
            staked: acc.staked.into(),
            unstaked: acc.unstaked.into(),
            available: acc.available.into(),
            /// shares * share_price - total = benefits
            benefits: benefits.into(),
            historic_benefits: (benefits + acc.retired_benefits).into()
        }
    }
}

