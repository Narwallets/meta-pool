use crate::*;
use near_sdk::{near_bindgen, Promise};

const MODE_DISTRIBUTE:u8=0;
const MODE_REBUILD:u8=1;

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

        //let epoch_height = env::epoch_height();
        // if self.last_epoch_height == epoch_height {
        //     return false;
        // }
        // self.last_epoch_height = epoch_height;

        //----------
        //check if the liquidity pool needs liquidity, and then use this opportunity to liquidate stnear in the LP by internal-clearing 
        if self.nslp_try_liquidate_stnear_by_clearing(){
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
            if sp.unstaked > 0 {
                //pool has unstaked amount
                if sp.unstaked < amount_to_stake {
                    //re-stake the unstaked
                    amount_to_stake = sp.unstaked;
                }
                //launch async stake to re-stake on the pool
                assert!(self.total_unstaked_and_waiting >= amount_to_stake,"total_unstaked_and_waiting < amount_to_stake");
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

            //here the sp has no unstaked balance, we must deposit_and_stake on the sp
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
    /// Called after amount is staked from the sp's unstaked balance (all into  the staking pool contract).
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
            if !included_deposit {
                //not deposited first, so staked funds came from unstaked funds already in the sp
                sp.unstaked -= amount;
            }
            //move into staked
            sp.staked += amount;
        } 
        else {
            //STAKE FAILED
            result = "has failed";
            if !included_deposit { //was staking from "wating for unstake"
                self.total_unstaked_and_waiting += amount; //undo preventive action considering the amount taken from wating for unstake
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

        let (sp_inx, amount_to_unstake) = self.get_staking_pool_requiring_unstake(total_to_unstake);
        if amount_to_unstake > 0 {
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

    //------------------------------------------------------------------------
    //-- COMPUTE AND DISTRIBUTE STAKING REWARDS for a specific staking-pool --
    //------------------------------------------------------------------------
    // Operator method, but open to anyone. Should be called once per epoch per sp, after sp rewards distribution (ping)
    /// Retrieves total balance from the staking pool and remembers it internally.
    /// Also computes and distributes rewards operator and delegators
    /// this fn queries the staking pool (makes a cross-contract call)
    pub fn distribute_rewards(&mut self, sp_inx: u16) {
        self.query_current_stake(sp_inx,MODE_DISTRIBUTE)
    }
    
    //utility to rebuild stake information by asking the pool
    pub fn rebuild_stake_from_pool_information(&mut self, sp_inx: u16) {
        assert!(&env::predecessor_account_id()==&self.owner_account_id || &env::predecessor_account_id()==&self.operator_account_id,
            "Can only be called by the operator or the owner"
        );
        self.query_current_stake(sp_inx,MODE_REBUILD)
    }


    fn query_current_stake(&mut self, sp_inx: u16, mode:u8) {
        //Note: In order to make this contract independent from the operator
        //this fn is open to be called by anyone
        //self.assert_owner_calling();

        let inx = sp_inx as usize;
        assert!(inx < self.staking_pools.len());

        let sp = &mut self.staking_pools[inx];
        assert!(!sp.busy_lock, "sp is busy");

        let epoch_height = env::epoch_height();

        if mode==MODE_DISTRIBUTE {

            if  sp.staked == 0 && sp.unstaked == 0 {
                return;
            }

            if sp.last_asked_rewards_epoch_height == epoch_height {
                return;
            }
        }

        log!(
            "Fetching total balance from the staking pool @{}",
            sp.account_id
        );

        sp.busy_lock = true;

        //query our current balance (includes staking rewards)
        ext_staking_pool::get_account_total_balance(
            env::current_account_id(),
            //promise params
            &sp.account_id,
            NO_DEPOSIT,
            gas::staking_pool::GET_ACCOUNT_TOTAL_BALANCE,
        )
        .then(ext_self_owner::on_get_sp_total_balance(
            inx,
            mode,
            //promise params
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_GET_SP_TOTAL_BALANCE,
        ));
    }

    /// prev fn continues here
    /*
    Note: what does the tag #[callback] applied to a fn in paramter do?
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
        mode: u8,
        #[callback] total_balance: U128String,
    ) {
        //we enter here after asking the staking-pool how much do we have staked (plus rewards)
        //total_balance: U128String contains the answer from the staking-pool

        assert_callback_calling();

        let rewards: u128;

        //store the new staked amount for this pool
        let new_staked_amount: u128;
        let sp = &mut self.staking_pools[sp_inx];

        sp.busy_lock = false;

        sp.last_asked_rewards_epoch_height = env::epoch_height();

        new_staked_amount = total_balance.0;

        if mode==MODE_REBUILD {
            log!(
                "REBUILD sp:{} staked:{}",
                sp.account_id, new_staked_amount
            );
            sp.staked = new_staked_amount;
            return; //***************
        }
        
        if new_staked_amount < sp.staked {
            log!(
                    "INCONSISTENCY @{} says total_balance < sp.staked",
                    sp.account_id
            );
            rewards = 0;
        } else {
            //compute rewards, as new balance minus old balance
            rewards = new_staked_amount - sp.staked;
        }

        log!(
            "sp:{} old_balance:{} new_balance:{} rewards:{}",
            sp.account_id, sp.staked, new_staked_amount, rewards
        );

        //updated accumulated_staked_rewards value for the contract
        self.accumulated_staked_rewards+=rewards;
        //updated new "staked" value for this pool
        sp.staked = new_staked_amount;
    
        if rewards > 0 {
            //add to actually staked
            self.total_actually_staked += rewards;

            // The fee that the contract owner (operator) takes.
            let owner_fee = apply_pct(self.operator_rewards_fee_basis_points, rewards);
            // The fee that the contract authors take.
            let developers_fee = apply_pct(DEVELOPERS_REWARDS_FEE_BASIS_POINTS, rewards);
            // Now add fees & shares to the pool preserving current share value
            // adds to self.total_actually_staked, self.total_for_staking & self.total_stake_shares;
            &self.add_amount_and_shares_preserve_share_price(self.operator_account_id.clone(),owner_fee);
            &self.add_amount_and_shares_preserve_share_price(DEVELOPERS_ACCOUNT_ID.into(), developers_fee);

            // rest of rewards go into total_actually_staked increasing share value for all stNEAR holders
            assert!(rewards > developers_fee + owner_fee);
            self.total_for_staking += rewards - developers_fee - owner_fee; //increase share price for everybody

        }
    }

    //----------------------------------------------------------------------
    // Operator method, but open to anyone
    //----------------------------------------------------------------------
    /// finds a pool with the unstake delay completed
    /// withdraws. Returns pol index or:
    /// -1 if there are funds ready to retrieve but the pool is busy
    /// -2 if there funds unstaked, but not ready in this epoch
    /// -3 if there are no unstaked funds
    pub fn get_staking_pool_requiring_retrieve(&self) -> i32 {
        
        let mut result:i32 = -3;

        for (sp_inx, sp) in self.staking_pools.iter().enumerate() {
            // if the pool is not busy, has stake, and has not unstaked blanace waiting for withdrawal
            if sp.unstaked > 0  {
                if result == -3 { result = -2};
                if sp.unstk_req_epoch_height + NUM_EPOCHS_TO_UNLOCK <= env::epoch_height() {
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
    /// launchs a withdrawal call
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
        assert!(env::epoch_height() >= sp.unstk_req_epoch_height + NUM_EPOCHS_TO_UNLOCK,
            "unstaking-delay ends at {}, now is {}",sp.unstk_req_epoch_height + NUM_EPOCHS_TO_UNLOCK,env::epoch_height());

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
    pub fn on_staking_pool_withdraw(&mut self, inx: u16) -> u128 {

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
            withdrawn_amount = amount;
        } 
        else {
            result = "has failed";
        }
        log!(
                "The withdrawal of {} from @{} {}",
                amount, &sp.account_id, result
        );
        return withdrawn_amount;
    }

}