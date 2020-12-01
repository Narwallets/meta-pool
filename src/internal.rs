use crate::*;
use near_sdk::{near_bindgen, Promise, Balance};

pub use crate::types::*;
pub use crate::utils::*;

/****************************/
/* general Internal methods */
/****************************/
impl DiversifiedPool {

    /// Asserts that the method was called by the owner.
    pub fn assert_owner(&self) {
        assert_eq!(
            &env::predecessor_account_id(),
            &self.owner_account_id,
            "Can only be called by the owner"
        )
    }

}

pub fn assert_min_amount(amount:u128) {
    assert!(amount>=TEN_NEAR,"minimun amount is 10N");
}

/***************************************/
/* Internal methods staking-pool trait */
/***************************************/
#[near_bindgen]
impl DiversifiedPool {

    pub(crate) fn internal_deposit(&mut self) {
        
        let amount = env::attached_deposit();
        assert_min_amount(amount);
        
        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);
        account.available += amount;
        self.internal_save_account(&account_id, &account);
        
        self.total_available += amount;

        env::log(
            format!(
                "@{} deposited {}. New available balance is {}",
                account_id, amount, account.available
            )
            .as_bytes(),
        );
    }

    //------------------------------
    pub(crate) fn internal_withdraw(&mut self, amount: u128) {
        
        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);

        if amount!=acc.available {
            assert_min_amount(amount);
        }

        assert!(
            acc.available >= amount,
            "Not enough available balance to withdraw the requested amount"
        );
        acc.available -= amount;
        self.internal_save_account(&account_id, &acc);

        Promise::new(account_id).transfer(amount);
        self.total_available -= amount;
    }

    //------------------------------
    pub(crate) fn internal_stake(&mut self, amount: Balance) {
        
        assert_min_amount(amount);

        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);

        assert!(
            acc.available >= amount,
            "Not enough available balance to stake the requested amount"
        );

        // Calculate the number of "stake" shares that the account will receive for staking the given amount.
        let num_shares = self.num_shares_from_amount(amount);
        assert!(num_shares>0);

        acc.stake_shares += num_shares;
        acc.available -= amount;
        self.internal_save_account(&account_id, &acc);

        self.total_stake_shares += num_shares;
        self.total_available -= amount;
        self.total_for_staking += amount;

    }

    //------------------------------
    pub(crate) fn inner_unstake(&mut self, amount_requested: u128) {

        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);

        let skash = self.amount_from_shares(acc.stake_shares);

        if skash>=TEN_NEAR {
            assert_min_amount(amount_requested);
        }

        assert!(
            skash >= amount_requested,
            "You have less skash than what you requested"
        );
        let remains_staked = skash - amount_requested;
        let amount_to_unstake = match remains_staked > ONE_NEAR { 
                true => amount_requested,
                false => skash, //unstake all 
            };

        let num_shares:u128;
        //if unstake all staked near, we use all shares, so we include rewards in the unstaking...
        //when "unstaking_all" the amount unstaked is the requested amount PLUS ALL ACCUMULATED REWARDS
        if amount_to_unstake == skash {
            num_shares = acc.stake_shares;
        }
        else {
            // Calculate the number of shares required to unstake the given amount.
            num_shares = self.num_shares_from_amount(amount_to_unstake);
            assert!(num_shares>0);
            assert!(
                acc.stake_shares >= num_shares,
                "inconsistency. Not enough shares to unstake"
            );
        }

        acc.stake_shares -= num_shares;
        acc.unstaked += amount_to_unstake;
        acc.unstaked_requested_epoch_height = env::epoch_height(); //when the unstake was requested

        //trip-meter
        acc.trip_accum_unstakes += amount_to_unstake;
        //--SAVE ACCOUNT--
        self.internal_save_account(&account_id, &acc);

        //--contract totals
        self.total_for_unstaking += amount_to_unstake;
        self.total_stake_shares -= num_shares;
        self.total_for_staking -= amount_to_unstake;

        // env::log(
        //     format!(
        //         "@{} unstaking {}. Spent {} staking shares. Total {} unstaked balance and {} staking shares",
        //         account_id, receive_amount, num_shares, account.unstaked, account.stake_shares
        //     )
        //         .as_bytes(),
        // );
        // env::log(
        //     format!(
        //         "Contract total staked balance is {}. Total number of shares {}",
        //         self.total_staked_balance, self.total_stake_shares
        //     )
        //     .as_bytes(),
        // );
    }



    //--------------------------------
    pub fn add_amount_and_shares_preserve_share_price(&mut self, account_id: AccountId, amount:u128){
        if amount>0 {
            let num_shares = self.num_shares_from_amount(amount);
            if num_shares > 0 {
                let account = &mut self.internal_get_account(&account_id);
                account.stake_shares += num_shares;
                &self.internal_save_account(&account_id, &account);
                // Increasing the total amount of "stake" shares.
                self.total_stake_shares += num_shares;
                self.total_for_staking += amount;
            }
        }
    }

    /// Returns the number of "stake" shares corresponding to the given near amount at current share_price
    /// if the amount & the shares are incorporated, price remains the same
    ///
    /// price = total_staked / total_shares
    /// Price is fixed
    /// (total_staked + amount) / (total_shares + num_shares) = total_staked / total_shares
    /// (total_staked + amount) * total_shares = total_staked * (total_shares + num_shares)
    /// amount * total_shares = total_staked * num_shares
    /// num_shares = amount * total_shares / total_staked
    pub(crate) fn num_shares_from_amount(
        &self,
        amount: Balance ) -> u128 
    {
        if amount==0||self.total_for_staking==0 {
            return 0;
        }
        return (U256::from(self.total_stake_shares) * U256::from(amount) / U256::from(self.total_for_staking)).as_u128();
    }

    /// Returns the amount corresponding to the given number of "stake" shares.
    pub(crate) fn amount_from_shares(
        &self,
        num_shares: u128 ) -> Balance 
    {
        if self.total_stake_shares == 0 || num_shares==0 {
            return 0;
        };
        return (U256::from(self.total_for_staking) * U256::from(num_shares) / U256::from(self.total_stake_shares)).as_u128();
    }


    /// Inner method to get the given account or a new default value account.
    pub(crate) fn internal_get_account(&self, account_id: &AccountId) -> Account {
        self.accounts.get(account_id).unwrap_or_default()
    }

    /// Inner method to save the given account for a given account ID.
    /// If the account balances are 0, the account is deleted instead to release storage.
    pub(crate) fn internal_save_account(&mut self, account_id: &AccountId, account: &Account) {
        if account.available==0 && account.unstaked==0 && account.stake_shares==0 {
            self.accounts.remove(account_id);
        } else {
            self.accounts.insert(account_id, &account);
        }
    }


//----------------------------------
// Heartbeat & Talking to the pools
// ---------------------------------

    /// checks if there's a pending wtihdrawal from any of the pools 
    /// and then launchs a withdrawal call
    pub(crate) fn internal_async_withdraw_from_a_pool(&mut self){
        
        let current_epoch = env::epoch_height();

        for (sp_inx,sp) in self.staking_pools.iter_mut().enumerate() {
            // if the pool is not busy, and we unstaked and the waiting period has elapsed
            if !sp.busy_lock && sp.unstaked>0 && sp.unstaked_requested_epoch_height+NUM_EPOCHS_TO_UNLOCK <= current_epoch {

                sp.busy_lock = true;

                //launch withdraw
                ext_staking_pool::withdraw(
                    sp.unstaked.into(),
                    &sp.account_id,
                    NO_DEPOSIT,
                    gas::staking_pool::WITHDRAW,
                )
                .then(ext_self_owner::on_staking_pool_withdraw(
                    sp_inx,
                    &env::current_account_id(),
                    NO_DEPOSIT,
                    gas::owner_callbacks::ON_STAKING_POOL_WITHDRAW,
                ));

                break; //just one pool
    
            }
        }

    }

    //prev fn continues here
    /// This method needs to update staking pool busyLock
    pub fn on_staking_pool_withdraw(&mut self, sp_inx: usize) -> bool {

        assert_self();
        let sp = &mut self.staking_pools[sp_inx];
        sp.busy_lock = false;
        let amount = sp.unstaked; //we retrieved all

        let withdraw_succeeded = is_promise_success();

        let result:&str;
        if withdraw_succeeded {
            result="succeeded";
            sp.unstaked -= amount; //no more unstaked in the pool
            //move from total_actually_unstaked to total_actually_unstaked_and_retrieved
            assert!(self.total_actually_unstaked <= amount);
            self.total_actually_unstaked -= amount;
            self.total_actually_unstaked_and_retrieved += amount;
            //the amount stays in "total_actually_unstaked_and_retrieved" until the user calls complete_unstaking
        } else {
            result="has failed";
        }
        env::log(
            format!(
                "The withdrawal of {} from @{} {}",
                amount,
                &sp.account_id,
                result
            )
            .as_bytes(),
        );
        return withdraw_succeeded
    }

    /// finds a staking pool requireing some stake to get balanced
    /// WARN: returns usize::MAX if no pool requires staking/all are busy
    pub(crate) fn get_staking_pool_requiring_stake(&self) -> usize {

        let mut max_required_amount:u128 = 0;
        let mut selected_sp_inx: usize = usize::MAX;

        for (sp_inx,sp) in self.staking_pools.iter().enumerate() {
            // if the pool is not busy, and this pool can stake
            if !sp.busy_lock && sp.weigth_basis_points>0 {
                // if this pool has an unbalance requiring staking
                let should_have = apply_pct(sp.weigth_basis_points, self.total_for_staking);
                // this pool requires staking?
                if should_have > sp.staked {
                    // how much?
                    let require_amount = should_have - sp.staked;
                    // is this the most unbalanced pool so far?
                    if require_amount > max_required_amount {
                        max_required_amount = require_amount;
                        selected_sp_inx = sp_inx;
                    }
                }
            }
        }

        return selected_sp_inx;

    }

    /// finds a staking pool requireing some stake to get balanced
    /// WARN: returns usize::MAX if no pool requires staking/all are busy
    pub(crate) fn get_staking_pool_requiring_unstake(&self) -> usize {

        let mut max_required_amount:u128 = 0;
        let mut selected_sp_inx: usize = usize::MAX;

        for (sp_inx,sp) in self.staking_pools.iter().enumerate() {
            // if the pool is not busy, has stake, and has not unstaked blanace waiting for withdrawal
            if !sp.busy_lock && sp.staked>0 && sp.unstaked==0 {
                // if this pool has an unbalance requiring un-staking
                let should_have = apply_pct(sp.weigth_basis_points, self.total_for_staking);
                // does this pool requires un-staking? (has too much staked?)
                if sp.staked > should_have  {
                    // how much?
                    let require_amount = sp.staked - should_have;
                    // is this the most unbalanced pool so far?
                    if require_amount > max_required_amount {
                        max_required_amount = require_amount;
                        selected_sp_inx = sp_inx;
                    }
                }
            }
        }

        return selected_sp_inx;

    }

}
