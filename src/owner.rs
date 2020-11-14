use crate::*;
use near_sdk::{near_bindgen, Promise, PublicKey};

#[near_bindgen]
impl DiversifiedPool {

    /// OWNER'S METHOD
    ///
    /// Requires 125 TGas (5 * BASE_GAS)
    ///
    /// Retrieves total balance from the staking pool and remembers it internally.
    /// Also computes and distributes benefits to author, operator and delegators
    /// this queries the staking pool.
    pub fn refresh_staking_pool_benefits(&mut self, sp_inx_i32:i32) {

        assert!(sp_inx_i32>0);

        self.assert_owner();

        let sp_inx = sp_inx_i32 as usize;
        assert!(sp_inx < self.staking_pools.len());

        let sp = &mut self.staking_pools[sp_inx];
        assert!(!sp.busy_lock,"sp is busy");

        if sp.staked==0 || sp.busy_lock { 
            return;
        }

        let epoch_height = env::epoch_height();
        if sp.last_asked_benefits_epoch_height == epoch_height {
            return;
        }

        env::log(
            format!(
                "Fetching total balance from the staking pool @{}",
                sp.account_id
            )
            .as_bytes(),
        );

        sp.busy_lock = true;

        ext_staking_pool::get_account_total_balance(
            env::current_account_id(),
            &sp.account_id,
            NO_DEPOSIT,
            gas::staking_pool::GET_ACCOUNT_TOTAL_BALANCE,
        )
        .then(ext_self_owner::on_get_sp_total_balance(
            sp_inx,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_GET_SP_TOTAL_BALANCE,
        ));
    }

    /// prev fn continues here
    /*
    Note: what does #[callback] do?
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
    pub fn on_get_sp_total_balance(&mut self, sp_inx: usize, #[callback] total_balance: U128String) {

        assert_self();
        let benefits:u128;

        {
            let sp = &mut self.staking_pools[sp_inx];

            sp.busy_lock = false;

            // env::log(
            //     format!(
            //         "The current total balance on the staking pool is {}",
            //         total_balance.0
            //     )
            //     .as_bytes(),
            // );

            sp.last_asked_benefits_epoch_height = env::epoch_height();

            let sp_staked_plus_ben = total_balance.0;

            if sp_staked_plus_ben >= sp.staked{
                env::log("INCONSISTENCY sp says total_balance < sp.staked".as_bytes() );
            }

            benefits = sp_staked_plus_ben.saturating_sub(sp.staked);
        }
        
        if benefits > 0 {
            // The fee that the contract author takes.
            let mut author_fee = apply_pct(AUTHOR_MIN_FEE_BASIS_POINTS,benefits);
            // The fee that the contract owner (operator) takes.
            let mut owner_fee = apply_pct(self.owner_fee_basis_points, benefits);
            // author fee comes from the operator/owner fee
            if owner_fee>author_fee {
                owner_fee-=author_fee
            }
            else {
                author_fee = owner_fee;
                owner_fee=0;
            }

            // Now add fees & shares to the pool not altering current share price
            self.add_benefits_and_shares(AUTHOR_ACCOUNT_ID.into(), author_fee);
            self.add_benefits_and_shares(self.owner_account_id.clone(), owner_fee);

            // rest of benefits go into the pool increasing share price
            assert!(benefits > author_fee + owner_fee);
            self.total_staked_benefits += benefits - author_fee - owner_fee;

            let sp = &self.staking_pools[sp_inx];
            env::log(
                format!(
                    "Received total rewards of {} tokens from {}. Staked was = {}",
                    benefits, sp.account_id, sp.staked,
                ).as_bytes(),
            );
        }

        self.total_staked_benefits+=benefits;

    }

    /// OWNER'S METHOD
    ///
    /// Requires 50 TGas (2 * BASE_GAS)
    ///
    /// Adds full access key with the given public key to the account once the contract is empty
    /// (has no accounts)
    pub fn add_full_access_key(&mut self, new_public_key: Base58PublicKey) -> Promise {
        self.assert_owner();
        assert!(!self.busy_lock 
            && self.total_for_staking==0 
            && self.total_for_unstaking==0
            && self.accounts.len()==0,
            "contract still has accounts or work to do"
        );

        env::log(b"Adding a full access key");

        let new_public_key: PublicKey = new_public_key.into();

        Promise::new(env::current_account_id()).add_full_access_key(new_public_key)
    }
}
