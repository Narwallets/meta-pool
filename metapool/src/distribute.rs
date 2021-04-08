use crate::*;
use near_sdk::{near_bindgen, Promise};

#[near_bindgen]
impl MetaPool {

    //----------------------------------
    // Heartbeat & Talking to the pools
    // ---------------------------------

    //-----------------------------
    // DISTRIBUTE
    //-----------------------------

    /// operator method -------------------------------------------------
    /// distribute_staking(). Do staking in batches of at most 100Kn
    /// returns "true" if the operator needs to call this fn again
    pub fn distribute_staking(&mut self) -> bool {

        //Note: In order to make this contract independent from the operator
        //this fn is open to be called by anyone

        //----------
        //check if the liquidity pool needs liquidity, and then use this opportunity to liquidate stnear in the LP by internal-clearing 
        if self.nslp_try_internal_clearing(){
            return true; //call again
        }

        //do wo need to stake?
        if self.total_for_staking <= self.total_actually_staked {
            log!("no staking needed");
            return false;
        }

        //-------------------------------------
        //compute amount to stake
        //-------------------------------------
        let total_amount_to_stake =  self.total_for_staking - self.total_actually_staked;
        let (sp_inx, mut amount_to_stake) = self.get_staking_pool_requiring_stake(total_amount_to_stake);
        log!("{} {} {}",total_amount_to_stake,sp_inx, amount_to_stake);
        if amount_to_stake > 0 {
            //most unbalanced pool found & available
            //launch async stake or deposit_and_stake on that pool

            let sp = &mut self.staking_pools[sp_inx];
            sp.busy_lock = true;

            //case 1. pool has unstaked amount (we could be at the unstaking delay waiting period)
            //NOTE: The amount to stake can't be so low as a few yoctos because the staking-pool 
            // will panic with : "panicked at 'The calculated number of \"stake\" shares received for staking should be positive', src/internal.rs:79:9"
            // that's because after division, if the amount is a few yoctos, the amount fo shares is 0
            if sp.unstaked >= TEN_NEAR { //at least 10 NEAR
                //pool has a sizable unstaked amount
                if sp.unstaked < amount_to_stake {
                    //re-stake the unstaked
                    amount_to_stake = sp.unstaked;
                }
                //launch async stake to re-stake in the pool
                assert!(self.total_unstaked_and_waiting >= amount_to_stake,"total_unstaked_and_waiting {} < amount_to_stake {}",self.total_unstaked_and_waiting,amount_to_stake);
                self.total_unstaked_and_waiting -= amount_to_stake; //preventively consider the amount removed from total_unstaked_and_waiting (undoes if failed)
                self.total_actually_staked += amount_to_stake; //preventively consider the amount staked (undoes if failed)
                ext_staking_pool::stake(
                    amount_to_stake.into(),
                    &sp.account_id,
                    NO_DEPOSIT,
                    gas::staking_pool::STAKE,
                )
                .then(ext_self_owner::on_staking_pool_stake_maybe_deposit(
                    sp_inx,
                    amount_to_stake,
                    false,
                    &env::current_account_id(),
                    NO_DEPOSIT,
                    gas::owner_callbacks::ON_STAKING_POOL_DEPOSIT_AND_STAKE,
                ));

                return true; //some work scheduled
            }

            //here the sp has no sizable unstaked balance, we must deposit_and_stake on the sp
            //launch async deposit_and_stake on the pool
            assert!(
                env::account_balance() - MIN_BALANCE_FOR_STORAGE >= amount_to_stake,
                "env::account_balance()-MIN_BALANCE_FOR_STORAGE < amount_to_stake"
            );

            self.total_actually_staked += amount_to_stake; //preventively consider the amount staked (undoes if async fails)
            ext_staking_pool::deposit_and_stake(
                &sp.account_id,
                amount_to_stake.into(), //attached amount
                gas::staking_pool::DEPOSIT_AND_STAKE,
            )
            .then(ext_self_owner::on_staking_pool_stake_maybe_deposit(
                sp_inx,
                amount_to_stake,
                true,
                &env::current_account_id(),
                NO_DEPOSIT,
                gas::owner_callbacks::ON_STAKING_POOL_DEPOSIT_AND_STAKE,
            ));

        }

        return true; //more work needed

    }

    //prev fn continues here
    /// Called after amount is staked into a staking-pool
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_stake_maybe_deposit(
        &mut self,
        sp_inx: usize,
        amount: u128,
        included_deposit: bool,
    ) -> bool {

        assert_callback_calling();

        let sp = &mut self.staking_pools[sp_inx];
        sp.busy_lock = false;

        let stake_succeeded = is_promise_success();

        let result: &str;
        if stake_succeeded {
            //STAKED OK
            result = "succeeded";
            if included_deposit { //we send NEAR to the staking-pool
                self.contract_account_balance -= amount;
            }
            else {
                //not deposited first, so staked funds came from unstaked funds already in the staking-pool
                sp.unstaked -= amount;
            }
            //move into staked
            sp.staked += amount;
        } 
        else {
            //STAKE FAILED
            result = "has failed";
            if !included_deposit { //was staking from "waiting for unstake"
                self.total_unstaked_and_waiting += amount; //undo preventive action considering the amount taken from waiting for unstake
            }
            self.total_actually_staked -= amount; //undo preventive action considering the amount staked
        }
        log!("Staking of {} at @{} {}", amount, sp.account_id, result);
        return stake_succeeded;
    }


    // Operator method, but open to anyone
    /// distribute_unstaking(). Do unstaking 
    /// returns "true" if needs to be called again
    pub fn distribute_unstaking(&mut self) -> bool {

        //Note: In order to make this contract independent from the operator
        //this fn is open to be called by anyone

        //let epoch_height = env::epoch_height();
        // if self.last_epoch_height == epoch_height {
        //     return false;
        // }
        // self.last_epoch_height = epoch_height;

        //--------------------------
        //compute amount to unstake
        //--------------------------
        if self.total_actually_staked <= self.total_for_staking {
            //no unstaking needed
            return false;
        }
        let total_to_unstake = self.total_actually_staked - self.total_for_staking;
        //check if the amount justifies tx-fee 
        if total_to_unstake <= 10*TGAS { 
            return false;
        }

        let (sp_inx, amount_to_unstake) = self.get_staking_pool_requiring_unstake(total_to_unstake);
        if amount_to_unstake > 10*TGAS { //only if the amount justifies tx-fee 
            //most unbalanced pool found & available
            //launch async to unstake

            let sp = &mut self.staking_pools[sp_inx];
            sp.busy_lock = true;

            //preventively consider the amount un-staked (undoes if promise fails)
            self.total_actually_staked -= amount_to_unstake; 
            self.total_unstaked_and_waiting += amount_to_unstake; 
            //launch async to un-stake from the pool
            ext_staking_pool::unstake(
                amount_to_unstake.into(),
                &sp.account_id,
                NO_DEPOSIT,
                gas::staking_pool::UNSTAKE,
            )
            .then(ext_self_owner::on_staking_pool_unstake(
                sp_inx,
                amount_to_unstake,
                &env::current_account_id(),
                NO_DEPOSIT,
                gas::owner_callbacks::ON_STAKING_POOL_UNSTAKE,
            ));

        }

        return true; //needs to be called again
    }

    /// The prev fn continues here
    /// Called after the given amount was unstaked at the staking pool contract.
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_unstake(&mut self, sp_inx: usize, amount: u128) -> bool {

        assert_callback_calling();

        let sp = &mut self.staking_pools[sp_inx];
        sp.busy_lock = false;

        let unstake_succeeded = is_promise_success();

        let result: &str;
        if unstake_succeeded {
            result = "succeeded";
            sp.staked -= amount;
            sp.unstaked += amount;
            sp.unstk_req_epoch_height = env::epoch_height();
        } else {
            result = "has failed";
            self.total_actually_staked += amount; //undo preventive action considering the amount unstaked
            self.total_unstaked_and_waiting -= amount; //undo preventive action considering the amount unstaked
        }

        log!("Unstaking of {} at @{} {}", amount, sp.account_id, result);
        return unstake_succeeded;
    }
    
    //--FIXES
    //utility to rebuild stake information if it goes out-of-sync
    pub fn rebuild_stake_from_pool_information(&mut self, sp_inx: u16, staked:U128String, unstaked:U128String) {
        
        self.assert_operator_or_owner();

        let inx = sp_inx as usize;
        assert!(inx < self.staking_pools.len());

        let sp = &mut self.staking_pools[inx];
        assert!(!sp.busy_lock, "sp is busy");

        sp.staked = staked.0;
        sp.unstaked = unstaked.0;

    }

    //-- If extra balance has accumulated (30% of tx fees by near-protocol)
    pub fn extra_balance_accumulated(&self) -> U128String {
        return env::account_balance().saturating_sub(self.contract_account_balance).into();
    }

    //-- If extra balance has accumulated (30% of tx fees by near-protocol)
    // transfer to self.operator_account_id
    pub fn transfer_extra_balance_accumulated(&mut self){
        let extra_balance  = self.extra_balance_accumulated().0;
        if extra_balance >= ONE_NEAR {
            Promise::new(self.operator_account_id.clone()).transfer(extra_balance);
        }
    }
    
    //--FIXES
    //utility to rebuild stake information if it goes out-of-sync
    pub fn rebuild_contract_staked(&mut self, total_actually_staked:U128String, total_unstaked_and_waiting:U128String) {
        self.assert_operator_or_owner();
        self.total_actually_staked = total_actually_staked.0;
        self.total_unstaked_and_waiting = total_unstaked_and_waiting.0;
    }

    //--FIXES
    //utility to rebuild stake information if it goes out-of-sync
    pub fn rebuild_contract_available(&mut self, total_available:U128String, total_actually_unstaked_and_retrieved:U128String ) {
        self.assert_operator_or_owner();
        self.total_available = total_available.0;
        self.total_actually_unstaked_and_retrieved = total_actually_unstaked_and_retrieved.0;
    }

    //--FIXES
    //utility to rebuild information if it goes out-of-sync
    pub fn set_contract_account_balance(&mut self, contract_account_balance:U128String ) {
        self.assert_operator_or_owner();
        self.contract_account_balance = contract_account_balance.0;
    }

    //--FIXES
    // in the simplified user flow, there's no more "available". All is staked or withdrew
    // for old accounts, stake the available
    //------------------------------
    pub fn stake_available(&mut self, account_id: AccountId) {

        self.assert_operator_or_owner();

        let mut acc = self.internal_get_account(&account_id);

        //take from the account "available" balance
        let amount = acc.in_memory_withdraw(acc.available, self);
        assert_min_amount(amount);

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

        //--SAVE ACCOUNT--
        self.internal_update_account(&account_id, &acc);

        //----------
        //check if the liquidity pool needs liquidity, and then use this opportunity to liquidate stnear in the LP by internal-clearing 
        self.nslp_try_internal_clearing();

    }


    //-------------------------
    /// sync_unstaked_balance: called before `retrieve_funds_from_a_pool`
    /// when you unstake, core-contracts/staking-pool does some share calculation *rounding*, so the real unstaked amount is not exactly 
    /// the same amount requested (a minor, few yoctoNEARS difference)
    /// this fn syncs sp.unstaked with the real, current unstaked amount informed by the sp
    pub fn sync_unstaked_balance(&mut self, sp_inx: u16) -> Promise {

        let inx = sp_inx as usize;
        assert!(inx < self.staking_pools.len());

        let sp = &mut self.staking_pools[inx];
        assert!(!sp.busy_lock, "sp is busy");

        sp.busy_lock = true;

        //query our current unstaked amount
        return ext_staking_pool::get_account_unstaked_balance(
            env::current_account_id(),
            //promise params
            &sp.account_id,
            NO_DEPOSIT,
            gas::staking_pool::GET_ACCOUNT_TOTAL_BALANCE,
        )
        .then(ext_self_owner::on_get_sp_unstaked_balance(
            inx,
            //promise params
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_GET_SP_TOTAL_BALANCE,
        ));
    }

    /// prev fn continues here - sync_unstaked_balance
    //------------------------------
    pub fn on_get_sp_unstaked_balance(
        &mut self,
        sp_inx: usize,
        #[callback] unstaked_balance: U128String ) 
    {
        //we enter here after asking the staking-pool how much do we have *unstaked*
        //unstaked_balance: U128String contains the answer from the staking-pool

        assert_callback_calling();

        let sp = &mut self.staking_pools[sp_inx];
        sp.busy_lock = false;

        // real unstaked amount for this pool
        let real_unstaked_balance: u128 = unstaked_balance.0;

        log!(
            "inx:{} sp:{} old_unstaked_balance:{} new_unstaked_balance:{}",
            sp_inx, sp.account_id, sp.unstaked, real_unstaked_balance
        );
        if real_unstaked_balance > sp.unstaked {
            //positive difference
            let difference = real_unstaked_balance - sp.unstaked;
            log!("positive difference {}",difference);
            sp.unstaked = real_unstaked_balance;
            assert!(sp.staked>=difference);
            sp.staked -= difference; //the difference was in "our" record of "staked"
        }
        else if real_unstaked_balance < sp.unstaked {
            //negative difference
            let difference = sp.unstaked - real_unstaked_balance ;
            log!("negative difference {}",difference);
            sp.unstaked = real_unstaked_balance;
            sp.staked += difference; //the difference was in "our" record of "staked"
        }

    }

    //------------------------------------------------------------------------
    //-- COMPUTE AND DISTRIBUTE STAKING REWARDS for a specific staking-pool --
    //------------------------------------------------------------------------
    // Operator method, but open to anyone. Should be called once per epoch per sp, after sp rewards distribution (ping)
    /// Retrieves total balance from the staking pool and remembers it internally.
    /// Also computes and distributes rewards for operator and stakers
    /// this fn queries the staking pool (makes a cross-contract call)
    pub fn distribute_rewards(&mut self, sp_inx: u16) {
        //Note: In order to make this contract independent from the operator
        //this fn is open to be called by anyone
        //self.assert_owner_calling();

        let inx = sp_inx as usize;
        assert!(inx < self.staking_pools.len());

        let sp = &mut self.staking_pools[inx];
        assert!(!sp.busy_lock, "sp is busy");

        let epoch_height = env::epoch_height();

        if  sp.staked == 0 && sp.unstaked == 0 {
            return;
        }

        if sp.last_asked_rewards_epoch_height == epoch_height {
            return;
        }

        log!(
            "Fetching total balance from the staking pool @{}",
            sp.account_id
        );

        sp.busy_lock = true;

        //query our current balance (includes staked+unstaked+staking rewards)
        ext_staking_pool::get_account_total_balance(
            env::current_account_id(),
            //promise params
            &sp.account_id,
            NO_DEPOSIT,
            gas::staking_pool::GET_ACCOUNT_TOTAL_BALANCE,
        )
        .then(ext_self_owner::on_get_sp_total_balance(
            inx,
            //promise params
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_GET_SP_TOTAL_BALANCE,
        ));
    }

    /// prev fn continues here
    /*
    Note: what does the tag #[callback] applied to a fn in parameter do?
    #[callback] parses the previous promise's result into the param
        Check out https://nomicon.io/RuntimeSpec/Components/BindingsSpec/PromisesAPI.html
        1. check promise_results_count() == 1
        2  check the execution status of the first promise and write the result into the register using promise_result(0, register_id) == 1
            Let's say that you used register_id == 0
        3. read register using register_len and read_register into Wasm memory
        4. parse the data using: let total_balance: WrappedBalance = serde_json::from_slice(&buf).unwrap();

    it has be last argument? can you add another argument for the on_xxx callback ?
    before that
    for example:
        /// Called after the request to get the current total balance from the staking pool.
        pub fn on_get_account_total_balance(&mut self, staking_pool_account: AccountId, #[callback] total_balance: WrappedBalance) {
            assert_self();
            self.set_staking_pool_status(TransactionStatus::Idle);
            ...
        and in the call
            ext_staking_pool::get_account_total_balance(
                env::current_account_id(),
                staking_pool_account_id,
                NO_DEPOSIT,
                gas::staking_pool::GET_ACCOUNT_TOTAL_BALANCE,
            )
            .then(ext_self_owner::on_get_account_total_balance(
                staking_pool_account_id,
                &env::current_account_id(),
                NO_DEPOSIT,
                gas::owner_callbacks::ON_GET_ACCOUNT_TOTAL_BALANCE,
            ))

    #[callback] marked-arguments are parsed in order. The position within arguments are not important, but the order is.
    If you have 2 arguments marked as #[callback] then you need to expect 2 promise results joined with promise_and
    */

    //------------------------------
    pub fn on_get_sp_total_balance(
        &mut self,
        sp_inx: usize,
        #[callback] total_balance: U128String,
    ) {
        //we enter here after asking the staking-pool how much do we have staked (plus rewards)
        //total_balance: U128String contains the answer from the staking-pool

        assert_callback_calling();

        let rewards: u128;

        //store the new staked amount for this pool
        let new_total_balance: u128;
        let sp = &mut self.staking_pools[sp_inx];

        sp.busy_lock = false;

        sp.last_asked_rewards_epoch_height = env::epoch_height();

        //total_balance is staking-pool.staked + staking-pool.unstaked
        new_total_balance = total_balance.0;

        if new_total_balance < sp.total_balance() {
            log!(
                    "INCONSISTENCY @{} says new_total_balance < our info sp.total_balance()",
                    sp.account_id
            );
            rewards = 0;
        } else {
            //compute rewards, as new balance minus old balance
            rewards = new_total_balance - sp.total_balance();
        }

        log!(
            "sp:{} old_balance:{} new_balance:{} rewards:{}",
            sp.account_id, sp.total_balance(), new_total_balance, rewards
        );

        //updated accumulated_staked_rewards value for the contract
        self.accumulated_staked_rewards+=rewards;
        //updated new "staked" value for this pool
        sp.staked = new_total_balance - sp.unstaked;
    
        if rewards > 0 {

            //add to total_for_staking & total_actually_staked, increasing share value for all stNEAR holders
            self.total_actually_staked += rewards;
            self.total_for_staking += rewards;

            // mint extra stNEAR representing fees for owner & developers
            // The fee the owner takes from rewards (0.5%)
            let operator_fee = apply_pct(self.operator_rewards_fee_basis_points, rewards);
            let operator_fee_shares = self.stake_shares_from_amount(operator_fee);
            // The fee the contract authors take from rewards (0.2%)
            let developers_fee = apply_pct(DEVELOPERS_REWARDS_FEE_BASIS_POINTS, rewards);
            let developers_fee_shares = self.stake_shares_from_amount(developers_fee);
            // Now add the newly minted shares. The fee is taken by making share price increase slightly smaller
            &self.add_extra_minted_shares(self.operator_account_id.clone(),operator_fee_shares);
            &self.add_extra_minted_shares(DEVELOPERS_ACCOUNT_ID.into(), developers_fee_shares);

        }
    }

    //----------------------------------------------------------------------
    // Operator method, but open to anyone
    //----------------------------------------------------------------------
    /// finds a pool with the unstake delay completed
    /// withdraws. Returns `sp_index` or:
    /// -1 if there are funds ready to retrieve but the pool is busy
    /// -2 if there funds unstaked, but not ready in this epoch
    /// -3 if there are no unstaked funds
    pub fn get_staking_pool_requiring_retrieve(&self) -> i32 {
        
        let mut result:i32 = -3;

        for (sp_inx, sp) in self.staking_pools.iter().enumerate() {
            // if the pool is not busy, has stake, and has not unstaked balance waiting for withdrawal
            if sp.unstaked > 10*TGAS { //if the to retrieve amount justifies the tx-fee
                if result == -3 { result = -2};
                if sp.wait_period_ended() {
                    if result == -2 { result = -1};
                    if !sp.busy_lock {
                        // if this pool has unstaked and the waiting period has ended
                        return sp_inx as i32;
                    }
                }
            }
        }
        return result;

    }

    // Operator method, but open to anyone
    //----------------------------------------------------------------------
    //  WITHDRAW FROM ONE OF THE POOLS ONCE THE WAITING PERIOD HAS ELAPSED
    //----------------------------------------------------------------------
    /// launches a withdrawal call
    /// returns the amount withdrawn
    /// call get_staking_pool_requiring_retrieve first
    /// 
    pub fn retrieve_funds_from_a_pool(&mut self, inx:u16) -> Promise {

        //Note: In order to make fund-recovering independent from the operator
        //this fn is open to be called by anyone

        assert!(inx < self.staking_pools.len() as u16,"invalid index");

        let sp = &mut self.staking_pools[inx as usize];
        assert!(!sp.busy_lock,"sp is busy");
        assert!(sp.unstaked > 0,"sp unstaked == 0");
        if !sp.wait_period_ended() {
            panic!("unstaking-delay ends at {}, now is {}", sp.unstk_req_epoch_height + NUM_EPOCHS_TO_UNLOCK, env::epoch_height());
        }

        // if the pool is not busy, and we unstaked and the waiting period has elapsed
        sp.busy_lock = true;

        //return promise
        return ext_staking_pool::withdraw(
            sp.unstaked.into(),
            //promise params:
            &sp.account_id,
            NO_DEPOSIT,
            gas::staking_pool::WITHDRAW,
        )
        .then(ext_self_owner::on_staking_pool_withdraw(
            inx,
            //promise params:
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_STAKING_POOL_WITHDRAW,
        ));
        
    }

    //prev fn continues here
    /// This method needs to update staking pool busyLock
    pub fn on_staking_pool_withdraw(&mut self, inx: u16) -> U128String {

        assert_callback_calling();

        let sp = &mut self.staking_pools[inx as usize];
        sp.busy_lock = false;
        
        let amount = sp.unstaked; //we retrieved all

        let withdraw_succeeded = is_promise_success();
        let mut withdrawn_amount:u128=0;

        let result: &str;
        if withdraw_succeeded {
            result = "succeeded";
            sp.unstaked = sp.unstaked.saturating_sub(amount); //no more unstaked in the pool
            //move from total_unstaked_and_waiting to total_actually_unstaked_and_retrieved
            self.total_unstaked_and_waiting = self.total_unstaked_and_waiting.saturating_sub(amount);
            self.total_actually_unstaked_and_retrieved += amount; //the amount stays in "total_actually_unstaked_and_retrieved" until the user calls finish_unstaking
            self.contract_account_balance += amount;
            withdrawn_amount = amount;
        } 
        else {
            result = "has failed";
        }
        log!(
            "The withdrawal of {} from @{} {}",
            amount, &sp.account_id, result
        );
        return withdrawn_amount.into();
    }

}