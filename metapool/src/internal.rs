use crate::*;
use near_sdk::{Balance, Promise, log};

pub use crate::types::*;
pub use crate::utils::*;

/****************************/
/* general Internal methods */
/****************************/
impl MetaPool {
    /// Asserts that the method was called by the owner.
    pub fn assert_owner_calling(&self) {
        assert_eq!(
            &env::predecessor_account_id(),
            &self.owner_account_id,
            "Can only be called by the owner"
        )
    }
    pub fn assert_operator_or_owner(&self) {
        assert!(&env::predecessor_account_id()==&self.owner_account_id || &env::predecessor_account_id()==&self.operator_account_id,
            "Can only be called by the operator or the owner"
        );
    }

    pub fn assert_not_busy(&self) {
        assert!(!self.contract_busy, "Contract is busy. Try again later");
    }

}

pub fn assert_min_amount(amount: u128) {
    assert!(amount >= NEAR, "minimum amount is 1N");
}


/***************************************/
/* Internal methods staking-pool trait */
/***************************************/
impl MetaPool {

    pub(crate) fn internal_deposit(&mut self) {
        assert_min_amount(env::attached_deposit());
        self.internal_deposit_attached_near_into(env::predecessor_account_id());
    }

    pub(crate) fn internal_deposit_attached_near_into(&mut self, account_id:AccountId) {

        let amount = env::attached_deposit();

        let mut account = self.internal_get_account(&account_id);

        account.available += amount;
        self.total_available += amount;
        self.contract_account_balance += amount;

        self.internal_update_account(&account_id, &account);

        log!("{} deposited into @{}'s account. New available balance is {}",
            amount, account_id, account.available
            );

    }

    //------------------------------
    // MIMIC staking-pool, if there are unstaked, it must be free to withdraw
    pub(crate) fn internal_withdraw_use_unstaked(&mut self, requested_amount: u128) -> Promise {
        self.inner_withdraw(requested_amount, true)
    }
    //------------------------------
    pub(crate) fn internal_withdraw_from_available(&mut self, requested_amount: u128) -> Promise {
        self.inner_withdraw(requested_amount, false)
    }
    //------------------------------
    fn inner_withdraw(&mut self, amount_requested: u128, must_use_unstaked:bool) -> Promise {
        
        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);

        if must_use_unstaked && account.unstaked>0 { //MIMIC staking-pool, if there is some unstaked, it must be free to withdraw
            account.in_memory_try_finish_unstaking(&account_id, self);
        }

        let amount = account.take_from_available(amount_requested, self);

        //commented: Remove min_account_balance requirements, increase liq-pool target to  cover all storage requirements
        //2 reasons: a) NEAR storage was cut by 10  b) in the simplified flow, users do not keep "available" balance
        // assert!( !acc.is_empty() || acc.available >= self.min_account_balance,
        //     "The min balance for an open account is {} NEAR. You need to close the account to remove all funds",
        //     self.min_account_balance/NEAR);

        self.internal_update_account(&account_id, &account);
        //transfer to user native near account
        
        return self.native_transfer_to_predecessor(amount);
    }
    
    pub(crate) fn native_transfer_to_predecessor(&mut self, amount: u128) -> Promise {
        //transfer to user native near account
        self.contract_account_balance -= amount;
        return Promise::new(env::predecessor_account_id()).transfer(amount);
    }

    //------------------------------
    /// takes from account.available and mints stNEAR
    /// actual stake ins staking-pool is made by the meta-pool-heartbeat before the end of the epoch
    pub(crate) fn internal_stake(&mut self, amount_requested: Balance) {

        self.assert_not_busy();

        assert_min_amount(amount_requested);

        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);

        //take from the account "available" balance
        let amount = acc.take_from_available(amount_requested, self);

        //use this operation to realize meta pending rewards
        acc.stake_realize_meta(self);
    
        // Calculate the number of "stake" shares that the account will receive for staking the given amount.
        let num_shares = self.stake_shares_from_amount(amount);
        assert!(num_shares > 0);

        //add shares to user account
        acc.add_stake_shares(num_shares, amount);
        //contract totals
        self.total_stake_shares += num_shares;
        self.total_for_staking += amount;
        self.epoch_stake_orders += amount;

        //--SAVE ACCOUNT--
        self.internal_update_account(&account_id, &acc);

        //log event 
        event!(r#"{{"event":"STAKE","account":"{}","amount":"{}"}}"#, account_id, amount);

        //----------
        //check if the liquidity pool needs liquidity, and then use this opportunity to liquidate stnear in the LP by internal-clearing 
        self.nslp_try_internal_clearing();

    }

    //------------------------------
    pub(crate) fn internal_unstake(&mut self, amount_requested: u128) {

        self.assert_not_busy();

        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);

        let valued_shares = self.amount_from_stake_shares(acc.stake_shares);

        let amount_to_unstake:u128;
        let stake_shares_to_burn:u128;
        // if the amount is close to user's total, remove user's total
        // to: a) do not leave less than ONE_MILLI_NEAR in the account, b) Allow 10 yoctos of rounding, e.g. remove(100) removes 99.999993 without panicking
        if is_close(amount_requested, valued_shares) { // allow for rounding simplification
            amount_to_unstake = valued_shares;
            stake_shares_to_burn =  acc.stake_shares; // close enough to all shares, burn-it all (avoid leaving "dust")
        }
        else {
            //use amount_requested
            amount_to_unstake = amount_requested;
            // Calculate the number shares that the account will burn based on the amount requested
            stake_shares_to_burn = self.stake_shares_from_amount(amount_requested);
        }

        assert!( valued_shares >= amount_to_unstake, "Not enough stNEAR {} to unstake the requested amount", valued_shares );
        assert!(stake_shares_to_burn > 0 && stake_shares_to_burn <= acc.stake_shares);
        
        //use this operation to realize meta pending rewards
        acc.stake_realize_meta(self);

        //burn acc stake shares
        acc.sub_stake_shares(stake_shares_to_burn, amount_to_unstake);
        //the amount is now "unstaked", i.e. the user has a claim to this amount, 4-8 epochs form now
        acc.unstaked += amount_to_unstake;
        acc.unstaked_requested_unlock_epoch = env::epoch_height() + self.internal_compute_current_unstaking_delay(amount_to_unstake); //when the unstake will be available
        //--contract totals
        self.epoch_unstake_orders += amount_to_unstake;
        self.total_stake_shares -= stake_shares_to_burn;
        self.total_for_staking -= amount_to_unstake;

        //--SAVE ACCOUNT--
        self.internal_update_account(&account_id, &acc);

        event!(r#"{{"event":"D-UNSTK","account_id":"{}","amount":"{}","shares":"{}"}}"#, account_id, amount_to_unstake, stake_shares_to_burn);

        log!(
                "@{} unstaked {}. Has now {} unstaked and {} stNEAR. Epoch:{}",
                account_id, amount_to_unstake, acc.unstaked, self.amount_from_stake_shares(acc.stake_shares), env::epoch_height()
        );
    }

    //--------------------------------------------------
    /// adds liquidity from deposited amount
    pub(crate) fn internal_nslp_add_liquidity(&mut self, amount_requested: u128) -> u16 {

        self.assert_not_busy();

        let account_id = env::predecessor_account_id();
        let mut acc = self.internal_get_account(&account_id);

        //take from the account "available" balance
        let amount = acc.take_from_available(amount_requested, self);

        //get NSLP account
        let mut nslp_account = self.internal_get_nslp_account();

        //use this LP operation to realize meta pending rewards (same as nslp_harvest_meta)
        acc.nslp_realize_meta(&nslp_account, self);

        // Calculate the number of "nslp" shares that the account will receive for adding the given amount of near liquidity
        let num_shares = self.nslp_shares_from_amount(amount, &nslp_account);
        assert!(num_shares > 0);

        //register added liquidity to compute rewards correctly
        acc.lp_meter.stake(amount);

        //update user account
        acc.nslp_shares += num_shares;
        //update NSLP account & main
        nslp_account.available += amount;
        self.total_available += amount;
        nslp_account.nslp_shares += num_shares; //total nslp shares

        //compute the % the user now owns of the Liquidity Pool (in basis points)
        let result_bp = proportional(10_000, acc.nslp_shares, nslp_account.nslp_shares) as u16;

        //--SAVE ACCOUNTS
        self.internal_update_account(&account_id, &acc);
        self.internal_save_nslp_account(&nslp_account);

        event!(r#"{{"event":"ADD.L","account_id":"{}","amount":"{}"}}"#, account_id, amount);

        return result_bp;
    }

    //--------------------------------------------------
    /// computes unstaking delay on current situation
    pub fn internal_compute_current_unstaking_delay(&self, amount:u128) -> u64 {
        let mut normal_wait_staked_available:u128 =0;
        for (_,sp) in self.staking_pools.iter().enumerate() {
            //if the pool has no unstaking in process
            if !sp.busy_lock && sp.staked>0 && sp.wait_period_ended() { 
                normal_wait_staked_available += sp.staked;
                if normal_wait_staked_available > amount {
                    return NUM_EPOCHS_TO_UNLOCK 
                }
            }
        }
        //all pools are in unstaking-delay, it will take double the time
        return 2 * NUM_EPOCHS_TO_UNLOCK; 
    }


    //--------------------------------
    pub(crate) fn add_extra_minted_shares(
        &mut self,
        account_id: AccountId,
        num_shares: u128,
    ) {
        if num_shares > 0 {
            let account = &mut self.internal_get_account(&account_id);
            account.stake_shares += num_shares;
            &self.internal_update_account(&account_id, &account);
            // Increasing the total amount of stake shares (reduces price)
            self.total_stake_shares += num_shares;
        }
    }

    /// Returns the number of "stake" shares corresponding to the given near amount at current share_price
    /// if the amount & the shares are incorporated, price remains the same
    pub(crate) fn stake_shares_from_amount(&self, amount: Balance) -> u128 {
        return shares_from_amount(amount, self.total_for_staking, self.total_stake_shares);
    }

    /// Returns the amount corresponding to the given number of "stake" shares.
    pub(crate) fn amount_from_stake_shares(&self, num_shares: u128) -> u128 {
        return amount_from_shares(num_shares, self.total_for_staking, self.total_stake_shares);
    }

    //-----------------------------
    // NSLP: NEAR/stNEAR Liquidity Pool
    //-----------------------------

    // NSLP shares are trickier to compute since the NSLP itself can have stNEAR
    pub(crate) fn nslp_shares_from_amount(&self, amount: u128, nslp_account: &Account) -> u128 {
        let total_pool_value: u128 = nslp_account.available
            + self.amount_from_stake_shares(nslp_account.stake_shares);
        return shares_from_amount(amount, total_pool_value, nslp_account.nslp_shares);
    }

    // NSLP shares are trickier to compute since the NSLP itself can have stNEAR
    pub(crate) fn amount_from_nslp_shares(&self, num_shares: u128, nslp_account: &Account) -> u128 {
        let total_pool_value: u128 = nslp_account.available
            + self.amount_from_stake_shares(nslp_account.stake_shares);
        return amount_from_shares(num_shares, total_pool_value, nslp_account.nslp_shares);
    }

    //----------------------------------
    // The LP acquires stNEAR providing the liquid-unstake service
    // The LP needs to remove stNEAR automatically, to recover liquidity and to keep the fee low.
    // The LP can recover near by internal clearing.
    // returns true if it uses clearing to liquidate
    // ---------------------------------
    pub(crate) fn nslp_try_internal_clearing(&mut self) -> bool {
        if self.total_for_staking <= self.total_actually_staked {
            //nothing ordered to be actually staked
            return false;
        }
        let amount_to_stake:u128 =  self.total_for_staking - self.total_actually_staked;
        let mut nslp_account = self.internal_get_nslp_account();
        log!("nslp internal clearing nslp_account.stake_shares {}",nslp_account.stake_shares);
        if nslp_account.stake_shares > 0 {
            //how much stnear does the nslp have?
            let valued_stake_shares = self.amount_from_stake_shares(nslp_account.stake_shares);
            //how much can we liquidate?
            let (shares_to_liquidate, amount_to_liquidate) =
                if amount_to_stake >= valued_stake_shares  { 
                    ( nslp_account.stake_shares, valued_stake_shares )
                } 
                else { 
                    ( self.stake_shares_from_amount(amount_to_stake), amount_to_stake )
                };
            //nslp sells-stnear directly, contract now needs to stake less
            log!("NSLP clearing {} {}",shares_to_liquidate, amount_to_liquidate);
            //log event 
            event!(r#"{{"event":"NSLP.clr","shares":"{}","amount":"{}"}}"#, shares_to_liquidate, amount_to_liquidate);

            nslp_account.sub_stake_shares(shares_to_liquidate, amount_to_liquidate);
            self.total_stake_shares -= shares_to_liquidate;
            self.total_for_staking -= amount_to_liquidate; //nslp has burned shares, total_for_staking is less now
            self.total_available += amount_to_liquidate; // amount returns to total_available (since it was never staked to begin with)
            nslp_account.available += amount_to_liquidate; //nslp has more available now
            //save account
            self.internal_save_nslp_account(&nslp_account);
            return true;
        }        
        return false;
    }

    /// computes discount_basis_points for NEAR/stNEAR Swap based on NSLP Balance
    pub(crate) fn internal_get_discount_basis_points(
        &self,
        available_near: u128,
        max_nears_to_pay: u128,
    ) -> u16 {
        log!(
            "get_discount_basis_points available_near={}  max_nears_to_pay={}",
            available_near, max_nears_to_pay
        );

        if available_near <= max_nears_to_pay {
            return self.nslp_max_discount_basis_points;
        }
        //amount after the swap
        let near_after = available_near - max_nears_to_pay;
        if near_after >= self.nslp_liquidity_target {
            //still >= target
            return self.nslp_min_discount_basis_points;
        }

        //linear curve from max to min on target
        let range  = self.nslp_max_discount_basis_points - self.nslp_min_discount_basis_points;
        //here 0<near_after<self.nslp_liquidity_target, so 0<proportional_bp<range
        let proportional_bp = proportional(range as u128, near_after, self.nslp_liquidity_target);

        return self.nslp_max_discount_basis_points - proportional_bp as u16;

    }

    /// user method - NEAR/stNEAR SWAP functions
    /// return how much NEAR you can get by selling x stNEAR
    pub(crate) fn internal_get_near_amount_sell_stnear(
        &self,
        available_near: u128,
        stnear_to_sell: u128,
    ) -> u128 {
        let discount_basis_points =
            self.internal_get_discount_basis_points(available_near, stnear_to_sell);
        assert!(discount_basis_points < 10000, "inconsistency d>1");
        let discount = apply_pct(discount_basis_points, stnear_to_sell);
        return (stnear_to_sell - discount).into(); //when stNEAR is sold user gets a discounted value because the user skips the waiting period

        // env::log(
        //     format!(
        //         "@{} withdrawing {}. New unstaked balance is {}",
        //         account_id, amount, account.unstaked
        //     )
        //     .as_bytes(),
        // );
    }

    /// Inner method to get the given account or a new default value account.
    pub(crate) fn internal_get_account(&self, account_id: &String) -> Account {
        self.accounts.get(account_id).unwrap_or_default()
    }

    /// Inner method to save the given account for a given account ID.
    /// If the account balances are 0, the account is deleted instead to release storage.
    pub(crate) fn internal_update_account(&mut self, account_id: &String, account: &Account) {
        if account.is_empty() {
            self.accounts.remove(account_id);
        } else {
            self.accounts.insert(account_id, &account); //insert_or_update
        }
    }

    /// Inner method to get the given account or a new default value account.
    pub(crate) fn internal_get_nslp_account(&self) -> Account {
        self.accounts
            .get(&NSLP_INTERNAL_ACCOUNT.into())
            .unwrap_or_default()
    }
    pub(crate) fn internal_save_nslp_account(&mut self, nslp_account: &Account) {
        self.internal_update_account(&NSLP_INTERNAL_ACCOUNT.into(), &nslp_account);
    }


    /// finds a staking pool requiring some stake to get balanced
    /// WARN: (returns 0,0) if no pool requires staking/all are busy
    pub(crate) fn get_staking_pool_requiring_stake(&self, total_to_stake:u128) -> (usize,u128) {
        let mut selected_to_stake_amount: u128 = 0;
        let mut selected_sp_inx:usize=0;

        for (sp_inx, sp) in self.staking_pools.iter().enumerate() {
            // if the pool is not busy, and this pool can stake
            if !sp.busy_lock && sp.weight_basis_points > 0 {
                // if this pool has an unbalance requiring staking
                let should_have = apply_pct(sp.weight_basis_points, self.total_for_staking);
                // this pool requires staking?
                if should_have > sp.staked {
                    // how much?
                    let require_amount = should_have - sp.staked;
                    // is this the most unbalanced pool so far?
                    if require_amount > selected_to_stake_amount {
                        selected_to_stake_amount = require_amount;
                        selected_sp_inx = sp_inx;
                    }
                }
            }
        }

        if selected_to_stake_amount>0 {
            //to avoid moving small amounts, if the remainder is less than 1K increase amount to include all in this movement
            if selected_to_stake_amount > total_to_stake { selected_to_stake_amount = total_to_stake };
            let remainder = total_to_stake - selected_to_stake_amount;
            if remainder <= MIN_STAKE_UNSTAKE_AMOUNT_MOVEMENT { 
                selected_to_stake_amount += remainder 
            };
        }

        return (selected_sp_inx, selected_to_stake_amount);
    }

    /// finds a staking pool requiring some stake to get balanced
    /// WARN: returns (0,0) if no pool requires staking/all are busy
    pub(crate) fn get_staking_pool_requiring_unstake(&self, total_to_unstake:u128) -> (usize,u128) {
        let mut selected_to_unstake_amount: u128 = 0;
        let mut selected_stake: u128 = 0;
        let mut selected_sp_inx: usize = 0;

        for (sp_inx, sp) in self.staking_pools.iter().enumerate() {
            // if the pool is not busy, has stake
            if !sp.busy_lock && sp.staked > 0 {
                //if has not unstaked balance waiting for withdrawal, or wait started in this same epoch (no harm in unstaking more)
                if sp.wait_period_ended() || sp.unstk_req_epoch_height==env::epoch_height() { 
                    // if this pool has an unbalance requiring un-staking
                    let should_have = apply_pct(sp.weight_basis_points, self.total_for_staking);
                    // does this pool requires un-staking? (has too much staked?)
                    if sp.staked > should_have {
                        // how much?
                        let unstake_amount = sp.staked - should_have;
                        // is this the most unbalanced pool so far?
                        if unstake_amount > selected_to_unstake_amount {
                            selected_to_unstake_amount = unstake_amount;
                            selected_stake = sp.staked;
                            selected_sp_inx = sp_inx;
                        }
                    }
                }
            }
        }

        if selected_to_unstake_amount>0 {
            if selected_to_unstake_amount > total_to_unstake { 
                selected_to_unstake_amount = total_to_unstake 
            };
            //to avoid moving small amounts, if the remainder is less than 5K and this pool can accommodate the unstaking, increase amount
            let remainder = total_to_unstake - selected_to_unstake_amount;
            if remainder <= MIN_STAKE_UNSTAKE_AMOUNT_MOVEMENT && selected_stake > selected_to_unstake_amount+remainder+2*MIN_STAKE_UNSTAKE_AMOUNT_MOVEMENT { 
                selected_to_unstake_amount += remainder 
            };
        }
        return (selected_sp_inx, selected_to_unstake_amount);
    }

    // MULTI FUN TOKEN [NEP-138](https://github.com/near/NEPs/pull/138)
    /// Transfer `amount` of tok tokens from the caller of the contract (`predecessor_id`) to `receiver_id`.
    /// Requirements:
    /// * receiver_id must pre-exist
    pub fn internal_multifuntok_transfer(&mut self, sender_id: &AccountId, receiver_id: &AccountId, symbol:&str, am: u128) {
        assert_ne!(
            sender_id, receiver_id,
            "Sender and receiver should be different"
        );
        assert!(am > 0, "The amount should be a positive number");
        let mut sender_acc = self.internal_get_account(&sender_id);
        let mut receiver_acc = self.internal_get_account(&receiver_id);
        match &symbol as &str {
            "NEAR" => {
                assert!(sender_acc.available >= am,"@{} not enough NEAR available {}",sender_id,sender_acc.available);
                sender_acc.available -= am;
                receiver_acc.available += am;
            }
            STNEAR => {
                let max_stnear = self.amount_from_stake_shares(sender_acc.stake_shares);
                assert!( am <= max_stnear,"@{} not enough stNEAR balance {}",sender_id,max_stnear);
                let shares = self.stake_shares_from_amount(am);
                sender_acc.sub_stake_shares(shares,am);
                receiver_acc.add_stake_shares(shares,am);
            }
            "META" => {
                sender_acc.stake_realize_meta(self);
                assert!(sender_acc.realized_meta >= am, "@{} not enough $META balance {}",sender_id,sender_acc.realized_meta);
                sender_acc.realized_meta -= am;
                receiver_acc.realized_meta += am;
            }
            _ => panic!("invalid symbol")
        }
        self.internal_update_account(&sender_id, &sender_acc);
        self.internal_update_account(&receiver_id, &receiver_acc);
    }

}
