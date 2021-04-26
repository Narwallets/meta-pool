use crate::*;
use near_sdk::{near_bindgen, Promise, log};

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

        self.assert_not_busy();

        //do we need to stake?
        if self.total_for_staking <= self.total_actually_staked {
            log!("no staking needed");
            return false;
        }

        //----------
        //check if the liquidity pool needs liquidity, and then use this opportunity to liquidate stnear in the LP by internal-clearing 
        if self.nslp_try_internal_clearing(){
            return true; //call again
        }

        //-------------------------------------
        //compute amount to stake
        //-------------------------------------
        
        //there could be minor yocto corrections after sync_unstake, altering total_actually_staked, consider that
        let  total_amount_to_stake =  std::cmp::min(self.epoch_stake_orders, self.total_for_staking - self.total_actually_staked);
        if total_amount_to_stake < MIN_MOVEMENT {
            log!("amount too low {}",total_amount_to_stake);
            return false;
        }
        
        // find pool
        let (sp_inx, mut amount_to_stake) = self.get_staking_pool_requiring_stake(total_amount_to_stake);
        log!("total_amount_to_stake:{} get_staking_pool_requiring_stake=>{},{}",total_amount_to_stake,sp_inx, amount_to_stake);
        if amount_to_stake > 0 {
            //most unbalanced pool found & available

            self.contract_busy=true;
            let sp = &mut self.staking_pools[sp_inx];
            sp.busy_lock = true;

            //case 1. pool has unstaked amount (we could be at the unstaking delay waiting period)
            //NOTE: The amount to stake can't be so low as a few yoctos because the staking-pool 
            // will panic with : "panicked at 'The calculated number of \"stake\" shares received for staking should be positive', src/internal.rs:79:9"
            // that's because after division, if the amount is a few yoctos, the amount for shares is 0
            if sp.unstaked >= TEN_NEAR { //at least 10 NEAR
                //pool has a sizable unstaked amount
                if sp.unstaked < amount_to_stake {
                    //re-stake the unstaked
                    amount_to_stake = sp.unstaked;
                }

                //schedule async stake to re-stake in the pool
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

            }

            else {

              //here the sp has no sizable unstaked balance, we must deposit_and_stake on the sp from our balance
              assert!( env::account_balance() - MIN_BALANCE_FOR_STORAGE >= amount_to_stake,"env::account_balance()-MIN_BALANCE_FOR_STORAGE < amount_to_stake");

              //schedule async stake or deposit_and_stake on that pool
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

        }

        //Here we did some staking (the promises are scheduled for exec after this fn completes)
        self.total_actually_staked += amount_to_stake; //preventively consider the amount staked (undoes if async fails)
        assert!(self.epoch_stake_orders>=amount_to_stake,"ISO epoch_stake_orders:{} amount_to_stake:{}",self.epoch_stake_orders,amount_to_stake);
        self.epoch_stake_orders-=amount_to_stake; //preventively reduce stake orders 
        //did some staking (promises scheduled), call again
        return true 

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

        let stake_succeeded = is_promise_success();

        let result: &str;
        if stake_succeeded {
            //STAKED OK
            result = "succeeded";
            let event:&str;
            if included_deposit { //we send NEAR to the staking-pool
                event="dist.stak"; //stake in the pools (including transfer)
                //we took from contract balance (transfer)
                self.contract_account_balance -= amount;
            }
            else {
                event="dist.stak.nt"; //stake in the pools, no-transfer
                //not deposited first, so staked funds came from unstaked funds already in the staking-pool
                sp.unstaked -= amount; //we've now less unstaked in this sp
                self.total_unstaked_and_waiting -= amount; // contract total of all unstaked & waiting
                //since we kept the NEAR in the contract and took from unstake-claims
                //reserve contract NEAR for the unstake-claims
                self.reserve_for_unstake_claims += amount;
            }
            //move into staked
            sp.staked += amount;
            //log event 
            event!(r#"{{"event":"{}","sp":"{}","amount":"{}"}}"#, event, sp.account_id, amount);

        } 
        else {
            //STAKE FAILED
            result = "has failed";
            self.total_actually_staked -= amount; //undo preventive action considering the amount staked
            self.epoch_stake_orders += amount; //undo preventively reduce stake orders 
        }
        log!("Staking of {} at @{} {}", amount, sp.account_id, result);

        //WARN: This is a callback after-cross-contract-call method
        //busy locks must be saved false in the state, this method SHOULD NOT PANIC
        sp.busy_lock = false;
        self.contract_busy=false;

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

        self.assert_not_busy();

        //--------------------------
        //compute amount to unstake
        //--------------------------
        if self.total_actually_staked <= self.total_for_staking {
            //no unstaking needed
            return false;
        }
        //there could be minor yocto corrections after sync_unstake, altering total_actually_staked, consider that
        let total_to_unstake = std::cmp::min(self.epoch_unstake_orders, self.total_actually_staked - self.total_for_staking);
        //check if the amount justifies tx-fee / can be unstaked really
        if total_to_unstake <= 10*TGAS as u128 { 
            return false;
        }

        let (sp_inx, amount_to_unstake) = self.get_staking_pool_requiring_unstake(total_to_unstake);
        if amount_to_unstake > 10*TGAS as u128 { //only if the amount justifies tx-fee 
            //most unbalanced pool found & available
            //launch async to unstake

            self.contract_busy=true;
            let sp = &mut self.staking_pools[sp_inx];
            sp.busy_lock = true;

            //preventively consider the amount un-staked (undoes if promise fails)
            assert!(self.total_actually_staked >= amount_to_unstake && self.epoch_unstake_orders >= amount_to_unstake,"IUN");
            self.total_actually_staked -= amount_to_unstake; 
            self.epoch_unstake_orders -= amount_to_unstake; 

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
                //extra async call args
                &env::current_account_id(),
                NO_DEPOSIT,
                gas::owner_callbacks::ON_STAKING_POOL_UNSTAKE,
            ));

            return true; //needs to be called again

        }
        else {
            return false;
        }

    }

    /// The prev fn continues here
    /// Called after the given amount was unstaked at the staking pool contract.
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_unstake(&mut self, sp_inx: usize, amount: u128) {

        assert_callback_calling();

        let sp = &mut self.staking_pools[sp_inx];

        let unstake_succeeded = is_promise_success();

        let result: &str;
        if unstake_succeeded {
            result = "succeeded";
            sp.staked -= amount;
            sp.unstaked += amount;
            sp.unstk_req_epoch_height = env::epoch_height();
            self.total_unstaked_and_waiting += amount; //contract total
            event!(r#"{{"event":"dist.unstk","sp":"{}","amount":"{}"}}"#, sp.account_id, amount);

        } else {
            result = "has failed";
            self.total_actually_staked += amount; //undo preventive action considering the amount unstaked
            self.epoch_unstake_orders += amount; //undo preventive action considering the amount unstaked
        }

        log!("Unstaking of {} at @{} {}", amount, sp.account_id, result);

        //WARN: This is a callback after-cross-contract-call method
        //busy locks must be saved false in the state, this method SHOULD NOT PANIC
        sp.busy_lock = false;
        self.contract_busy=false;

    }
    
    //utility to set contract busy flag manually by operator.
    #[payable]
    pub fn set_busy(&mut self, value: bool) {
        assert_one_yocto();
        self.assert_owner_calling();
        self.contract_busy=value;
    }
    //operator manual set sp.busy_lock
    #[payable]
    pub fn sp_busy(&mut self, sp_inx: u16, value:bool) {
        assert_one_yocto();
        self.assert_operator_or_owner();

        let inx = sp_inx as usize;
        assert!(inx < self.staking_pools.len());

        let sp = &mut self.staking_pools[inx];
        sp.busy_lock = value;

    }

    //-- check If extra balance has accumulated (30% of tx fees by near-protocol)
    pub fn extra_balance_accumulated(&self) -> U128String {
        return env::account_balance().saturating_sub(self.contract_account_balance).into();
    }

    //-- If extra balance has accumulated (30% of tx fees by near-protocol)
    // transfer to self.operator_account_id
    pub fn transfer_extra_balance_accumulated(&mut self) -> U128String {
        let extra_balance  = self.extra_balance_accumulated().0;
        if extra_balance >= ONE_NEAR {
            //only if there's more than one near, and left 10 cents (consider transfer fees)
            Promise::new(self.operator_account_id.clone()).transfer(extra_balance-10*NEAR_CENT);
            return extra_balance.into();
        }
        return 0.into();
    }

    //-------------------------
    /// sync_unstaked_balance: called before `retrieve_funds_from_a_pool`
    /// when you unstake, core-contracts/staking-pool does some share calculation *rounding*, so the real unstaked amount is not exactly 
    /// the same amount requested (a minor, few yoctoNEARS difference)
    /// this fn syncs sp.unstaked with the real, current unstaked amount informed by the sp
    pub fn sync_unstaked_balance(&mut self, sp_inx: u16) -> Promise {

        let inx = sp_inx as usize;
        assert!(inx < self.staking_pools.len());

        self.assert_not_busy();
        self.contract_busy=true;

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
            gas::owner_callbacks::ON_GET_SP_UNSTAKED_BALANCE,
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
            sp.staked = sp.staked.saturating_sub(difference); //the difference was in "our" record of "staked"
        }
        else if real_unstaked_balance < sp.unstaked {
            //negative difference
            let difference = sp.unstaked - real_unstaked_balance ;
            log!("negative difference {}",difference);
            sp.unstaked = real_unstaked_balance;
            sp.staked += difference; //the difference was in "our" record of "staked"
        }

        //WARN: This is a callback after-cross-contract-call method
        //busy locks must be saved false in the state, this method SHOULD NOT PANIC
        sp.busy_lock = false;
        self.contract_busy=false;

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

        self.assert_not_busy();

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

        self.contract_busy = true;
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

    pub fn on_get_sp_total_balance(
        &mut self,
        sp_inx: usize,
        #[callback] total_balance: U128String,
    ) {
        //we enter here after asking the staking-pool how much do we have staked (plus rewards)
        //total_balance: U128String contains the answer from the staking-pool

        assert_callback_calling();

        //store the new staked amount for this pool
        let new_total_balance: u128;
        let sp = &mut self.staking_pools[sp_inx];

        //WARN: This is a callback after-cross-contract-call method
        //busy locks must be saved false in the state, this method SHOULD NOT PANIC
        sp.busy_lock = false;
        self.contract_busy = false;

        sp.last_asked_rewards_epoch_height = env::epoch_height();

        //total_balance informed is staking-pool.staked + staking-pool.unstaked
        new_total_balance = total_balance.0;

        let rewards: u128;
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
            if sp.unstaked > 10*TGAS as u128 { //if the amount to retrieve justifies the tx-fee
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

        self.assert_not_busy();

        let sp = &mut self.staking_pools[inx as usize];
        assert!(!sp.busy_lock,"sp is busy");
        assert!(sp.unstaked > 0,"sp unstaked == 0");
        if !sp.wait_period_ended() {
            panic!("unstaking-delay ends at {}, now is {}", sp.unstk_req_epoch_height + NUM_EPOCHS_TO_UNLOCK, env::epoch_height());
        }

        // if we're here, the pool is not busy, and we unstaked and the waiting period has elapsed

        self.contract_busy = true;
        sp.busy_lock = true;

        //return promise
        return ext_staking_pool::withdraw(
            sp.unstaked.into(),
            //promise params:
            &sp.account_id,
            NO_DEPOSIT,
            gas::staking_pool::WITHDRAW,
        )
        .then(ext_self_owner::on_retrieve_from_staking_pool(
            inx,
            //promise params:
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_STAKING_POOL_WITHDRAW,
        ));
    }
    //prev fn continues here
    /// This method needs to update staking pool busyLock
    pub fn on_retrieve_from_staking_pool(&mut self, inx: u16) -> U128String {

        assert_callback_calling();

        let sp = &mut self.staking_pools[inx as usize];
        
        let amount = sp.unstaked; //we retrieved all

        let withdraw_succeeded = is_promise_success();

        let result: &str;
        let withdrawn_amount:u128;
        if withdraw_succeeded {
            result = "succeeded";
            withdrawn_amount = amount;
            sp.unstaked = sp.unstaked.saturating_sub(amount); //is no longer in the pool as "unstaked"
            self.total_unstaked_and_waiting = self.total_unstaked_and_waiting.saturating_sub(amount); //contract total
            // the amount is now in the contract balance
            self.contract_account_balance += amount;
            // the amount retrieved should be "reserved_for_unstaked_claims" until the user calls withdraw_unstaked
            self.reserve_for_unstake_claims += amount; 
            //log event 
            event!(r#"{{"event":"retrieve","sp":"{}","amount":"{}"}}"#, sp.account_id, amount);
        } 
        else {
            result = "has failed";
            withdrawn_amount = 0;
        }
        log!(
            "The withdrawal of {} from @{} {}",
            amount, &sp.account_id, result
        );

        //WARN: This is a callback after-cross-contract-call method
        //busy locks must be saved false in the state, this method SHOULD NOT PANIC
        sp.busy_lock = false;
        self.contract_busy = false;

        return withdrawn_amount.into();
    }

    // Operator method, but open to anyone
    //----------------------------------------------------------------------
    // End of Epoch clearing of STAKE_ORDERS vs UNSTAKE_ORDERS
    //----------------------------------------------------------------------
    // At the end of the epoch, only the delta between stake & unstake orders needs to be actually staked
    // if there are more in the stake orders than the unstake orders, some NEAR will not be sent to the pools
    // e.g. stake-orders: 1200, unstake-orders:1000 => net: stake 200 and keep 1000 to fulfill unstake claims after 4 epochs.
    // if there was more in the unstake orders than in the stake orders, a real unstake was initiated with one or more pools, 
    // the rest should also be kept to fulfill unstake claims after 4 epochs.
    // e.g. stake-orders: 700, unstake-orders:1000 => net: start-unstake 300 and keep 700 to fulfill unstake claims after 4 epochs
    // if the delta is 0, there's no real stake-unstake, but the amount should be kept to fulfill unstake claims after 4 epochs
    // e.g. stake-orders: 500, unstake-orders:500 => net: 0 so keep 500 to fulfill unstake claims after 4 epochs.
    //
    pub fn end_of_epoch_clearing(&mut self)  {

        self.assert_not_busy();

        if self.epoch_stake_orders==0 || self.epoch_unstake_orders==0 { return }

        let delta:u128;
        let to_keep:u128;
        if self.epoch_stake_orders >= self.epoch_unstake_orders {
            delta = self.epoch_stake_orders - self.epoch_unstake_orders;
            to_keep = self.epoch_stake_orders - delta;
        }
        else {
            delta = self.epoch_unstake_orders - self.epoch_stake_orders;
            to_keep = self.epoch_unstake_orders - delta;
        }

        //we will keep this NEAR (no need to send to the pools), but keep it reserved for unstake_claims, 4 epochs from now
        self.reserve_for_unstake_claims += to_keep;
        
        //clear opposing orders
        self.epoch_stake_orders -= to_keep;
        self.epoch_unstake_orders -= to_keep;

        self.epoch_last_clearing = env::epoch_height();
        event!(r#"{{"event":"clr.ord","keep":"{}"}}"#, to_keep);
    }

}