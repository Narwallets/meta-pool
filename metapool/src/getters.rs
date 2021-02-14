use crate::*;
use near_sdk::{near_bindgen};

#[near_bindgen]
impl MetaPool {
    //------------------------------------------
    // GETTERS 
    //------------------------------------------
    /// Returns the account ID of the owner.
    
    pub fn get_operator_account_id(&self) -> AccountId {
        return self.operator_account_id.clone();
    }

    /// The amount of tokens that were deposited to the staking pool.
    /// NOTE: The actual balance can be larger than this known deposit balance due to staking
    /// rewards acquired on the staking pool.
    /// To refresh the amount the owner can call `refresh_staking_pool_balance`.
    pub fn get_known_deposited_balance(&self) -> U128String {
        return self.total_actually_staked.into();
    }

    /// full account info
    /// Returns JSON representation of the account for the given account ID.
    pub fn get_account_info(&self, account_id: AccountId) -> GetAccountInfoResult {
        let acc = self.internal_get_account(&account_id);
        let stnear = self.amount_from_stake_shares(acc.stake_shares);
        // trip_rewards = current_stnear + trip_accum_unstakes - trip_accum_stakes - trip_start_stnear;
        let trip_rewards = (stnear + acc.trip_accum_unstakes).saturating_sub(acc.trip_accum_stakes + acc.trip_start_stnear);
        //NLSP share value
        let mut nslp_share_value: u128 = 0;
        if acc.nslp_shares != 0 {
            let nslp_account = self.internal_get_nslp_account();
            nslp_share_value = acc.valued_nslp_shares(self, &nslp_account);
        }
        return GetAccountInfoResult {
            account_id,
            available: acc.available.into(),
            stnear: stnear.into(),
            unstaked: acc.unstaked.into(),
            unstaked_requested_unlock_epoch: acc.unstaked_requested_unlock_epoch.into(),
            can_withdraw: (env::epoch_height() >= acc.unstaked_requested_unlock_epoch),
            total: (acc.available + stnear + acc.unstaked).into(),
            //trip-meter
            trip_start: acc.trip_start.into(),
            trip_start_stnear: acc.trip_start_stnear.into(),
            trip_accum_stakes: acc.trip_accum_stakes.into(),
            trip_accum_unstakes: acc.trip_accum_unstakes.into(),
            trip_rewards: trip_rewards.into(),

            nslp_shares: acc.nslp_shares.into(),
            nslp_share_value: nslp_share_value.into(),

            meta: acc.total_meta(self).into(),
        };
    }


    /// NEP-129 get information about this contract
    /// returns JSON string according to [NEP-129](https://github.com/nearprotocol/NEPs/pull/129)
    pub fn get_contract_info(&self) -> NEP129Response {
        return NEP129Response {
            dataVersion:1,
            name: CONTRACT_NAME.into(),
            version:CONTRACT_VERSION.into(),
            source:"https://github.com/Narwallets/meta-pool".into(), 
            standards:vec!("NEP-138".into(),"NEP-141".into()),  
            webAppUrl:self.web_app_url.clone(),
            developersAccountId:DEVELOPERS_ACCOUNT_ID.into(),
            auditorAccountId: self.auditor_account_id.clone()
        }
    }

    /// sets confirgurable contract info [NEP-129](https://github.com/nearprotocol/NEPs/pull/129)
    // Note: params are not Option<String> so the user can not inadvertely set null to data by not including the argument
    pub fn set_contract_info(&mut self, web_app_url:String, auditor_account_id:String) {
        self.assert_owner_calling();
        self.web_app_url = if web_app_url.len()>0 { Some(web_app_url) } else { None };
        self.auditor_account_id = if auditor_account_id.len()>0 { Some(auditor_account_id) } else { None };
    }

    /// get contract totals 
    /// Returns JSON representation of the contract state
    pub fn get_contract_state(&self) -> GetContractStateResult {

        let lp_account = self.internal_get_nslp_account();

        return GetContractStateResult {
            total_available: self.total_available.into(),
            total_for_staking: self.total_for_staking.into(),
            total_actually_staked: self.total_actually_staked.into(),
            accumulated_staked_rewards: self.accumulated_staked_rewards.into(),
            total_unstaked_and_waiting: self.total_unstaked_and_waiting.into(),
            total_actually_unstaked_and_retrieved: self.total_actually_unstaked_and_retrieved.into(),
            total_stake_shares: self.total_stake_shares.into(),
            total_meta: self.total_meta.into(),
            accounts_count: self.accounts.len().into(),
            staking_pools_count: self.staking_pools.len() as u16,
            nslp_liquidity: lp_account.available.into(),
            nslp_current_discount_basis_points: self.internal_get_discount_basis_points(lp_account.available, TEN_NEAR)
        };
    }

    /// Returns JSON representation of contract parameters
    pub fn get_contract_params(&self) -> ContractParamsJSON {
        return ContractParamsJSON {
            staking_paused: self.staking_paused,
            min_account_balance: self.min_account_balance.into(),

            nslp_near_target: self.nslp_near_target.into(),
            nslp_max_discount_basis_points: self.nslp_max_discount_basis_points,
            nslp_min_discount_basis_points: self.nslp_min_discount_basis_points,

            staker_meta_mult_pct: self.staker_meta_mult_pct,
            stnear_sell_meta_mult_pct: self.stnear_sell_meta_mult_pct,
            lp_provider_meta_mult_pct: self.lp_provider_meta_mult_pct,
                    
            operator_rewards_fee_basis_points: self.operator_rewards_fee_basis_points,
            operator_swap_cut_basis_points: self.operator_swap_cut_basis_points,
            treasury_swap_cut_basis_points: self.treasury_swap_cut_basis_points,
            };
    }

    /// Sets contract parameters 
    pub fn set_contract_params(&mut self, params:ContractParamsJSON) {

        self.assert_owner_calling();

        self.min_account_balance = params.min_account_balance.0;

        self.nslp_near_target = params.nslp_near_target.0;
        self.nslp_max_discount_basis_points = params.nslp_max_discount_basis_points;
        self.nslp_min_discount_basis_points = params.nslp_min_discount_basis_points;

        self.staker_meta_mult_pct = params.staker_meta_mult_pct;
        self.stnear_sell_meta_mult_pct = params.stnear_sell_meta_mult_pct;
        self.lp_provider_meta_mult_pct = params.lp_provider_meta_mult_pct;
                    
        self.operator_rewards_fee_basis_points = params.operator_rewards_fee_basis_points;
        self.operator_swap_cut_basis_points = params.operator_swap_cut_basis_points;
        self.treasury_swap_cut_basis_points = params.treasury_swap_cut_basis_points;

    }
    
    /// get sp (staking-pool) info
    /// Returns JSON representation of sp recorded state
    pub fn get_sp_info(&self, sp_inx_i32: i32) -> StakingPoolJSONInfo {

        assert!(sp_inx_i32 > 0);

        let sp_inx = sp_inx_i32 as usize;
        assert!(sp_inx < self.staking_pools.len());

        let sp = &self.staking_pools[sp_inx];

        return StakingPoolJSONInfo {
            account_id: sp.account_id.clone(),
            weight_basis_points: sp.weight_basis_points,
            staked: sp.staked.into(),
            unstaked: sp.unstaked.into(),
            unstaked_requested_epoch_height: sp.unstk_req_epoch_height.into(),
            last_asked_rewards_epoch_height: sp.last_asked_rewards_epoch_height.into(),
        };
    }
}
