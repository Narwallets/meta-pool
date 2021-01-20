use crate::*;
use near_sdk::{near_bindgen, Promise, PublicKey};

#[near_bindgen]
impl DiversifiedPool {
    /// OWNER'S METHOD
    ///
    /// Requires 125 TGas (5 * BASE_GAS)
    ///

    /// OWNER'S METHOD
    ///
    /// Requires 50 TGas (2 * BASE_GAS)
    ///
    /// Adds full access key with the given public key to the account once the contract is empty
    /// (has no accounts)
    pub fn add_full_access_key(&mut self, new_public_key: Base58PublicKey) -> Promise {
        
        self.assert_owner_calling();

        assert!(self.accounts.len() == 0, "contract still has accounts");

        env::log(b"Adding a full access key");

        let new_public_key: PublicKey = new_public_key.into();

        Promise::new(env::current_account_id()).add_full_access_key(new_public_key)
    }

    //---------------------------------
    // staking-pools-list (SPL) management
    //---------------------------------

    /// get the current list of pools
    pub fn get_staking_pool_list(&self) -> Vec<StakingPoolJSONInfo> {
        let mut result = Vec::with_capacity(self.staking_pools.len());
        for elem in self.staking_pools.iter(){
            result.push(StakingPoolJSONInfo{
                account_id: elem.account_id.clone(),
                weight_basis_points: elem.weight_basis_points,
                staked: elem.staked.into(),
                unstaked: elem.unstaked.into(),
                last_asked_rewards_epoch_height: elem.last_asked_rewards_epoch_height.into(),
                unstaked_requested_epoch_height: elem.unstk_req_epoch_height.into(),
            })
        }
        return result;
    }

    ///remove staking pool from list *if it's empty*
    pub fn remove_staking_pool(&mut self, inx:u16 ){

        self.assert_owner_calling();

        let sp = &self.staking_pools[inx as usize];
        if !sp.is_empty() {
            panic!(b"sp is not empty")
        }
        self.staking_pools.remove(inx as usize);
    }

    ///update existing weight_basis_points
    pub fn set_staking_pool_weight(&mut self, inx:u16, weight_basis_points:u16 ){

        self.assert_owner_calling();

        let sp = &mut self.staking_pools[inx as usize];
        if sp.busy_lock {
            panic!(b"sp is busy")
        }
        sp.weight_basis_points = weight_basis_points;
        self.check_staking_pool_list_consistency();
    }
    
    ///add a new staking pool or update existing weight_basis_points
    pub fn set_staking_pool(&mut self, account_id:AccountId, weight_basis_points:u16 ){

        self.assert_owner_calling();

        //search the pools
        for sp_inx in 0..self.staking_pools.len() {
            if self.staking_pools[sp_inx].account_id==account_id {
                //found, set weight_basis_points
                self.set_staking_pool_weight(sp_inx as u16, weight_basis_points);
                return;
            }
        }
        //not found, it's a new pool
        self.staking_pools.push(  StakingPoolInfo::new(account_id, weight_basis_points) );
        self.check_staking_pool_list_consistency();
    }

    fn check_staking_pool_list_consistency(&self) {
        assert!(self.sum_staking_pool_list_weight_basis_points()<=10000,"sum(staking_pools.weight) can not be GT 100%");
    }

    pub fn sum_staking_pool_list_weight_basis_points(&self) -> u16 {
        let mut total_weight: u16 = 0;
        for sp in self.staking_pools.iter() {
            total_weight+=sp.weight_basis_points;
        }
        return total_weight;
    }

    //--------------------------------------------------
    /// computes unstaking delay on current situation
    pub fn compute_current_unstaking_delay(&self, amount:U128String) -> u16 {
        return self.internal_compute_current_unstaking_delay(amount.0) as u16;
    }

}
