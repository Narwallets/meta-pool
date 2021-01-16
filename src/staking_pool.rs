use std::convert::TryInto;

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::json_types::{Base58PublicKey, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    env, ext_contract, near_bindgen, AccountId, Balance, EpochHeight, Promise, PromiseResult,
    PublicKey,
};
use uint::construct_uint;

/// The amount of gas given to complete `vote` call.
const VOTE_GAS: u64 = 100_000_000_000_000;

/// The amount of gas given to complete internal `on_stake_action` call.
const ON_STAKE_ACTION_GAS: u64 = 20_000_000_000_000;

/// The amount of yocto NEAR the contract dedicates to guarantee that the "share" price never
/// decreases. It's used during rounding errors for share -> amount conversions.
const STAKE_SHARE_PRICE_GUARANTEE_FUND: Balance = 1_000_000_000_000;

/// There is no deposit balance attached.
const NO_DEPOSIT: Balance = 0;

/// A type to distinguish between a balance and "stake" shares for better readability.
pub type NumStakeShares = Balance;

construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}

/// Inner account data of a delegate.
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq)]
pub struct Account {
    /// The unstaked balance. It represents the amount the account has on this contract that
    /// can either be staked or withdrawn.
    pub unstaked: Balance,
    /// The amount of "stake" shares. Every stake share corresponds to the amount of staked balance.
    /// NOTE: The number of shares should always be less or equal than the amount of staked balance.
    /// This means the price of stake share should always be at least `1`.
    /// The price of stake share can be computed as `total_staked_balance` / `total_stake_shares`.
    pub stake_shares: NumStakeShares,
    /// The minimum epoch height when the withdrawn is allowed.
    /// This changes after unstaking action, because the amount is still locked for 3 epochs.
    pub unstaked_available_epoch_height: EpochHeight,
}

/// Represents an account structure readable by humans.
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct HumanReadableAccount {
    pub account_id: AccountId,
    /// The unstaked balance that can be withdrawn or staked.
    pub unstaked_balance: U128,
    /// The amount balance staked at the current "stake" share price.
    pub staked_balance: U128,
    /// Whether the unstaked balance is available for withdrawal now.
    pub can_withdraw: bool,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            unstaked: 0,
            stake_shares: 0,
            unstaked_available_epoch_height: 0,
        }
    }
}

/// The number of epochs required for the locked balance to become unlocked.
/// NOTE: The actual number of epochs when the funds are unlocked is 3. But there is a corner case
/// when the unstaking promise can arrive at the next epoch, while the inner state is already
/// updated in the previous epoch. It will not unlock the funds for 4 epochs.
const NUM_EPOCHS_TO_UNLOCK: EpochHeight = 4;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakingContract {
    /// The account ID of the owner who's running the staking validator node.
    /// NOTE: This is different from the current account ID which is used as a validator account.
    /// The owner of the staking pool can change staking public key and adjust reward fees.
    pub owner_id: AccountId,
    /// The public key which is used for staking action. It's the public key of the validator node
    /// that validates on behalf of the pool.
    pub stake_public_key: PublicKey,
    /// The last epoch height when `ping` was called.
    pub last_epoch_height: EpochHeight,
    /// The last total balance of the account (consists of staked and unstaked balances).
    pub last_total_balance: Balance,
    /// The total amount of shares. It should be equal to the total amount of shares across all
    /// accounts.
    pub total_stake_shares: NumStakeShares,
    /// The total staked balance.
    pub total_staked_balance: Balance,
    /// The fraction of the reward that goes to the owner of the staking pool for running the
    /// validator node.
    pub reward_fee_fraction: RewardFeeFraction,
    /// Persistent map from an account ID to the corresponding account.
    pub accounts: UnorderedMap<AccountId, Account>,
    /// Whether the staking is paused.
    /// When paused, the account unstakes everything (stakes 0) and doesn't restake.
    /// It doesn't affect the staking shares or reward distribution.
    /// Pausing is useful for node maintenance. Only the owner can pause and resume staking.
    /// The contract is not paused by default.
    pub paused: bool,
}

impl Default for StakingContract {
    fn default() -> Self {
        env::panic(b"Staking contract should be initialized before usage")
    }
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct RewardFeeFraction {
    pub numerator: u32,
    pub denominator: u32,
}

impl RewardFeeFraction {
    pub fn assert_valid(&self) {
        assert_ne!(self.denominator, 0, "Denominator must be a positive number");
        assert!(
            self.numerator <= self.denominator,
            "The reward fee must be less or equal to 1"
        );
    }

    pub fn multiply(&self, value: Balance) -> Balance {
        (U256::from(self.numerator) * U256::from(value) / U256::from(self.denominator)).as_u128()
    }
}

/// Interface for a voting contract.
#[ext_contract(ext_voting)]
pub trait VoteContract {
    /// Method for validators to vote or withdraw the vote.
    /// Votes for if `is_vote` is true, or withdraws the vote if `is_vote` is false.
    fn vote(&mut self, is_vote: bool);
}

/// Interface for the contract itself.
#[ext_contract(ext_self)]
pub trait SelfContract {
    /// A callback to check the result of the staking action.
    /// In case the stake amount is less than the minimum staking threshold, the staking action
    /// fails, and the stake amount is not changed. This might lead to inconsistent state and the
    /// follow withdraw calls might fail. To mitigate this, the contract will issue a new unstaking
    /// action in case of the failure of the first staking action.
    fn on_stake_action(&mut self);
}

#[near_bindgen]
impl StakingContract {
    /// Initializes the contract with the given owner_id, initial staking public key (with ED25519
    /// curve) and initial reward fee fraction that owner charges for the validation work.
    ///
    /// The entire current balance of this contract will be used to stake. This allows contract to
    /// always maintain staking shares that can't be unstaked or withdrawn.
    /// It prevents inflating the price of the share too much.
    #[init]
    pub fn new(
        owner_id: AccountId,
        stake_public_key: Base58PublicKey,
        reward_fee_fraction: RewardFeeFraction,
    ) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        reward_fee_fraction.assert_valid();
        assert!(
            env::is_valid_account_id(owner_id.as_bytes()),
            "The owner account ID is invalid"
        );
        let account_balance = env::account_balance();
        let total_staked_balance = account_balance - STAKE_SHARE_PRICE_GUARANTEE_FUND;
        assert_eq!(
            env::account_locked_balance(),
            0,
            "The staking pool shouldn't be staking at the initialization"
        );
        let mut this = Self {
            owner_id,
            stake_public_key: stake_public_key.into(),
            last_epoch_height: env::epoch_height(),
            last_total_balance: account_balance,
            total_staked_balance,
            total_stake_shares: NumStakeShares::from(total_staked_balance),
            reward_fee_fraction,
            accounts: UnorderedMap::new(b"u".to_vec()),
            paused: false,
        };
        // Staking with the current pool to make sure the staking key is valid.
        this.internal_restake();
        this
    }

    /// Distributes rewards and restakes if needed.
    pub fn ping(&mut self) {
        if self.internal_ping() {
            self.internal_restake();
        }
    }

    /// Deposits the attached amount into the inner account of the predecessor.
    #[payable]
    pub fn deposit(&mut self) {
        let need_to_restake = self.internal_ping();

        self.internal_deposit();

        if need_to_restake {
            self.internal_restake();
        }
    }

    /// Deposits the attached amount into the inner account of the predecessor and stakes it.
    #[payable]
    pub fn deposit_and_stake(&mut self) {
        self.internal_ping();

        let amount = self.internal_deposit();
        self.internal_stake(amount);

        self.internal_restake();
    }

    /// Withdraws the entire unstaked balance from the predecessor account.
    /// It's only allowed if the `unstake` action was not performed in the four most recent epochs.
    pub fn withdraw_all(&mut self) {
        let need_to_restake = self.internal_ping();

        let account_id = env::predecessor_account_id();
        let account = self.internal_get_account(&account_id);
        self.internal_withdraw(account.unstaked);

        if need_to_restake {
            self.internal_restake();
        }
    }

    /// Withdraws the non staked balance for given account.
    /// It's only allowed if the `unstake` action was not performed in the four most recent epochs.
    pub fn withdraw(&mut self, amount: U128) {
        let need_to_restake = self.internal_ping();

        let amount: Balance = amount.into();
        self.internal_withdraw(amount);

        if need_to_restake {
            self.internal_restake();
        }
    }

    /// Stakes all available unstaked balance from the inner account of the predecessor.
    pub fn stake_all(&mut self) {
        // Stake action always restakes
        self.internal_ping();

        let account_id = env::predecessor_account_id();
        let account = self.internal_get_account(&account_id);
        self.internal_stake(account.unstaked);

        self.internal_restake();
    }

    /// Stakes the given amount from the inner account of the predecessor.
    /// The inner account should have enough unstaked balance.
    pub fn stake(&mut self, amount: U128) {
        // Stake action always restakes
        self.internal_ping();

        let amount: Balance = amount.into();
        self.internal_stake(amount);

        self.internal_restake();
    }

    /// Unstakes all staked balance from the inner account of the predecessor.
    /// The new total unstaked balance will be available for withdrawal in four epochs.
    pub fn unstake_all(&mut self) {
        // Unstake action always restakes
        self.internal_ping();

        let account_id = env::predecessor_account_id();
        let account = self.internal_get_account(&account_id);
        let amount = self.staked_amount_from_num_shares_rounded_down(account.stake_shares);
        self.inner_unstake(amount);

        self.internal_restake();
    }

    /// Unstakes the given amount from the inner account of the predecessor.
    /// The inner account should have enough staked balance.
    /// The new total unstaked balance will be available for withdrawal in four epochs.
    pub fn unstake(&mut self, amount: U128) {
        // Unstake action always restakes
        self.internal_ping();

        let amount: Balance = amount.into();
        self.inner_unstake(amount);

        self.internal_restake();
    }

    /****************/
    /* View methods */
    /****************/

    /// Returns the unstaked balance of the given account.
    pub fn get_account_unstaked_balance(&self, account_id: AccountId) -> U128 {
        self.get_account(account_id).unstaked_balance
    }

    /// Returns the staked balance of the given account.
    /// NOTE: This is computed from the amount of "stake" shares the given account has and the
    /// current amount of total staked balance and total stake shares on the account.
    pub fn get_account_staked_balance(&self, account_id: AccountId) -> U128 {
        self.get_account(account_id).staked_balance
    }

    /// Returns the total balance of the given account (including staked and unstaked balances).
    pub fn get_account_total_balance(&self, account_id: AccountId) -> U128 {
        let account = self.get_account(account_id);
        (account.unstaked_balance.0 + account.staked_balance.0).into()
    }

    /// Returns `true` if the given account can withdraw tokens in the current epoch.
    pub fn is_account_unstaked_balance_available(&self, account_id: AccountId) -> bool {
        self.get_account(account_id).can_withdraw
    }

    /// Returns the total staking balance.
    pub fn get_total_staked_balance(&self) -> U128 {
        self.total_staked_balance.into()
    }

    /// Returns account ID of the staking pool owner.
    pub fn get_owner_id(&self) -> AccountId {
        self.owner_id.clone()
    }

    /// Returns the current reward fee as a fraction.
    pub fn get_reward_fee_fraction(&self) -> RewardFeeFraction {
        self.reward_fee_fraction.clone()
    }

    /// Returns the staking public key
    pub fn get_staking_key(&self) -> Base58PublicKey {
        self.stake_public_key.clone().try_into().unwrap()
    }

    /// Returns true if the staking is paused
    pub fn is_staking_paused(&self) -> bool {
        self.paused
    }

    /// Returns human readable representation of the account for the given account ID.
    pub fn get_account(&self, account_id: AccountId) -> HumanReadableAccount {
        let account = self.internal_get_account(&account_id);
        HumanReadableAccount {
            account_id,
            unstaked_balance: account.unstaked.into(),
            staked_balance: self
                .staked_amount_from_num_shares_rounded_down(account.stake_shares)
                .into(),
            can_withdraw: account.unstaked_available_epoch_height <= env::epoch_height(),
        }
    }

    /// Returns the number of accounts that have positive balance on this staking pool.
    pub fn get_number_of_accounts(&self) -> u64 {
        self.accounts.len()
    }

    /// Returns the list of accounts
    pub fn get_accounts(&self, from_index: u64, limit: u64) -> Vec<HumanReadableAccount> {
        let keys = self.accounts.keys_as_vector();

        (from_index..std::cmp::min(from_index + limit, keys.len()))
            .map(|index| self.get_account(keys.get(index).unwrap()))
            .collect()
    }

    /*************/
    /* Callbacks */
    /*************/

    pub fn on_stake_action(&mut self) {
        assert_eq!(
            env::current_account_id(),
            env::predecessor_account_id(),
            "Can be called only as a callback"
        );

        assert_eq!(
            env::promise_results_count(),
            1,
            "Contract expected a result on the callback"
        );
        let stake_action_succeeded = match env::promise_result(0) {
            PromiseResult::Successful(_) => true,
            _ => false,
        };

        // If the stake action failed and the current locked amount is positive, then the contract
        // has to unstake.
        if !stake_action_succeeded && env::account_locked_balance() > 0 {
            Promise::new(env::current_account_id()).stake(0, self.stake_public_key.clone());
        }
    }

    /*******************/
    /* Owner's methods */
    /*******************/

    /// Owner's method.
    /// Updates current public key to the new given public key.
    pub fn update_staking_key(&mut self, stake_public_key: Base58PublicKey) {
        self.assert_owner();
        // When updating the staking key, the contract has to restake.
        let _need_to_restake = self.internal_ping();
        self.stake_public_key = stake_public_key.into();
        self.internal_restake();
    }

    /// Owner's method.
    /// Updates current reward fee fraction to the new given fraction.
    pub fn update_reward_fee_fraction(&mut self, reward_fee_fraction: RewardFeeFraction) {
        self.assert_owner();
        reward_fee_fraction.assert_valid();

        let need_to_restake = self.internal_ping();
        self.reward_fee_fraction = reward_fee_fraction;
        if need_to_restake {
            self.internal_restake();
        }
    }

    /// Owner's method.
    /// Calls `vote(is_vote)` on the given voting contract account ID on behalf of the pool.
    pub fn vote(&mut self, voting_account_id: AccountId, is_vote: bool) -> Promise {
        self.assert_owner();
        assert!(
            env::is_valid_account_id(voting_account_id.as_bytes()),
            "Invalid voting account ID"
        );

        ext_voting::vote(is_vote, &voting_account_id, NO_DEPOSIT, VOTE_GAS)
    }

    /// Owner's method.
    /// Pauses pool staking.
    pub fn pause_staking(&mut self) {
        self.assert_owner();
        assert!(!self.paused, "The staking is already paused");

        self.internal_ping();
        self.paused = true;
        Promise::new(env::current_account_id()).stake(0, self.stake_public_key.clone());
    }

    /// Owner's method.
    /// Resumes pool staking.
    pub fn resume_staking(&mut self) {
        self.assert_owner();
        assert!(self.paused, "The staking is not paused");

        self.internal_ping();
        self.paused = false;
        self.internal_restake();
    }
}

impl StakingContract {
    /********************/
    /* Internal methods */
    /********************/

    /// Restakes the current `total_staked_balance` again.
    pub(crate) fn internal_restake(&mut self) {
        if self.paused {
            return;
        }
        // Stakes with the staking public key. If the public key is invalid the entire function
        // call will be rolled back.
        Promise::new(env::current_account_id())
            .stake(self.total_staked_balance, self.stake_public_key.clone())
            .then(ext_self::on_stake_action(
                &env::current_account_id(),
                NO_DEPOSIT,
                ON_STAKE_ACTION_GAS,
            ));
    }

    pub(crate) fn internal_deposit(&mut self) -> u128 {
        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);
        let amount = env::attached_deposit();
        account.unstaked += amount;
        self.internal_save_account(&account_id, &account);
        self.last_total_balance += amount;

        env::log(
            format!(
                "@{} deposited {}. New unstaked balance is {}",
                account_id, amount, account.unstaked
            )
            .as_bytes(),
        );
        amount
    }

    pub(crate) fn internal_withdraw(&mut self, amount: Balance) {
        assert!(amount > 0, "Withdrawal amount should be positive");

        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);
        assert!(
            account.unstaked >= amount,
            "Not enough unstaked balance to withdraw"
        );
        assert!(
            account.unstaked_available_epoch_height <= env::epoch_height(),
            "The unstaked balance is not yet available due to unstaking delay"
        );
        account.unstaked -= amount;
        self.internal_save_account(&account_id, &account);

        env::log(
            format!(
                "@{} withdrawing {}. New unstaked balance is {}",
                account_id, amount, account.unstaked
            )
            .as_bytes(),
        );

        Promise::new(account_id).transfer(amount);
        self.last_total_balance -= amount;
    }

    pub(crate) fn internal_stake(&mut self, amount: Balance) {
        assert!(amount > 0, "Staking amount should be positive");

        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);

        // Calculate the number of "stake" shares that the account will receive for staking the
        // given amount.
        let num_shares = self.num_shares_from_staked_amount_rounded_down(amount);
        assert!(
            num_shares > 0,
            "The calculated number of \"stake\" shares received for staking should be positive"
        );
        // The amount of tokens the account will be charged from the unstaked balance.
        // Rounded down to avoid overcharging the account to guarantee that the account can always
        // unstake at least the same amount as staked.
        let charge_amount = self.staked_amount_from_num_shares_rounded_down(num_shares);
        assert!(
          charge_amount > 0,
          "Invariant violation. Calculated staked amount must be positive, because \"stake\" share price should be at least 1"
      );

        assert!(
            account.unstaked >= charge_amount,
            "Not enough unstaked balance to stake"
        );
        account.unstaked -= charge_amount;
        account.stake_shares += num_shares;
        self.internal_save_account(&account_id, &account);

        // The staked amount that will be added to the total to guarantee the "stake" share price
        // never decreases. The difference between `stake_amount` and `charge_amount` is paid
        // from the allocated STAKE_SHARE_PRICE_GUARANTEE_FUND.
        let stake_amount = self.staked_amount_from_num_shares_rounded_up(num_shares);

        self.total_staked_balance += stake_amount;
        self.total_stake_shares += num_shares;

        env::log(
          format!(
              "@{} staking {}. Received {} new staking shares. Total {} unstaked balance and {} staking shares",
              account_id, charge_amount, num_shares, account.unstaked, account.stake_shares
          )
              .as_bytes(),
      );
        env::log(
            format!(
                "Contract total staked balance is {}. Total number of shares {}",
                self.total_staked_balance, self.total_stake_shares
            )
            .as_bytes(),
        );
    }

    pub(crate) fn inner_unstake(&mut self, amount: u128) {
        assert!(amount > 0, "Unstaking amount should be positive");

        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);

        assert!(
            self.total_staked_balance > 0,
            "The contract doesn't have staked balance"
        );
        // Calculate the number of shares required to unstake the given amount.
        // NOTE: The number of shares the account will pay is rounded up.
        let num_shares = self.num_shares_from_staked_amount_rounded_up(amount);
        assert!(
          num_shares > 0,
          "Invariant violation. The calculated number of \"stake\" shares for unstaking should be positive"
      );
        assert!(
            account.stake_shares >= num_shares,
            "Not enough staked balance to unstake"
        );

        // Calculating the amount of tokens the account will receive by unstaking the corresponding
        // number of "stake" shares, rounding up.
        let receive_amount = self.staked_amount_from_num_shares_rounded_up(num_shares);
        assert!(
          receive_amount > 0,
          "Invariant violation. Calculated staked amount must be positive, because \"stake\" share price should be at least 1"
      );

        account.stake_shares -= num_shares;
        account.unstaked += receive_amount;
        account.unstaked_available_epoch_height = env::epoch_height() + NUM_EPOCHS_TO_UNLOCK;
        self.internal_save_account(&account_id, &account);

        // The amount tokens that will be unstaked from the total to guarantee the "stake" share
        // price never decreases. The difference between `receive_amount` and `unstake_amount` is
        // paid from the allocated STAKE_SHARE_PRICE_GUARANTEE_FUND.
        let unstake_amount = self.staked_amount_from_num_shares_rounded_down(num_shares);

        self.total_staked_balance -= unstake_amount;
        self.total_stake_shares -= num_shares;

        env::log(
          format!(
              "@{} unstaking {}. Spent {} staking shares. Total {} unstaked balance and {} staking shares",
              account_id, receive_amount, num_shares, account.unstaked, account.stake_shares
          )
              .as_bytes(),
      );
        env::log(
            format!(
                "Contract total staked balance is {}. Total number of shares {}",
                self.total_staked_balance, self.total_stake_shares
            )
            .as_bytes(),
        );
    }

    /// Asserts that the method was called by the owner.
    pub(crate) fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "Can only be called by the owner"
        );
    }

    /// Distributes rewards after the new epoch. It's automatically called before every action.
    /// Returns true if the current epoch height is different from the last epoch height.
    pub(crate) fn internal_ping(&mut self) -> bool {
        let epoch_height = env::epoch_height();
        if self.last_epoch_height == epoch_height {
            return false;
        }
        self.last_epoch_height = epoch_height;

        // New total amount (both locked and unlocked balances).
        // NOTE: We need to subtract `attached_deposit` in case `ping` called from `deposit` call
        // since the attached deposit gets included in the `account_balance`, and we have not
        // accounted it yet.
        let total_balance =
            env::account_locked_balance() + env::account_balance() - env::attached_deposit();

        assert!(
            total_balance >= self.last_total_balance,
            "The new total balance should not be less than the old total balance"
        );
        let total_reward = total_balance - self.last_total_balance;
        if total_reward > 0 {
            // The validation fee that the contract owner takes.
            let owners_fee = self.reward_fee_fraction.multiply(total_reward);

            // Distributing the remaining reward to the delegators first.
            let remaining_reward = total_reward - owners_fee;
            self.total_staked_balance += remaining_reward;

            // Now buying "stake" shares for the contract owner at the new share price.
            let num_shares = self.num_shares_from_staked_amount_rounded_down(owners_fee);
            if num_shares > 0 {
                // Updating owner's inner account
                let owner_id = self.owner_id.clone();
                let mut account = self.internal_get_account(&owner_id);
                account.stake_shares += num_shares;
                self.internal_save_account(&owner_id, &account);
                // Increasing the total amount of "stake" shares.
                self.total_stake_shares += num_shares;
            }
            // Increasing the total staked balance by the owners fee, no matter whether the owner
            // received any shares or not.
            self.total_staked_balance += owners_fee;

            env::log(
              format!(
                  "Epoch {}: Contract received total rewards of {} tokens. New total staked balance is {}. Total number of shares {}",
                  epoch_height, total_reward, self.total_staked_balance, self.total_stake_shares,
              )
                  .as_bytes(),
          );
            if num_shares > 0 {
                env::log(format!("Total rewards fee is {} stake shares.", num_shares).as_bytes());
            }
        }

        self.last_total_balance = total_balance;
        true
    }

    /// Returns the number of "stake" shares rounded down corresponding to the given staked balance
    /// amount.
    ///
    /// price = total_staked / total_shares
    /// Price is fixed
    /// (total_staked + amount) / (total_shares + num_shares) = total_staked / total_shares
    /// (total_staked + amount) * total_shares = total_staked * (total_shares + num_shares)
    /// amount * total_shares = total_staked * num_shares
    /// num_shares = amount * total_shares / total_staked
    pub(crate) fn num_shares_from_staked_amount_rounded_down(
        &self,
        amount: Balance,
    ) -> NumStakeShares {
        assert!(
            self.total_staked_balance > 0,
            "The total staked balance can't be 0"
        );
        (U256::from(self.total_stake_shares) * U256::from(amount)
            / U256::from(self.total_staked_balance))
        .as_u128()
    }

    /// Returns the number of "stake" shares rounded up corresponding to the given staked balance
    /// amount.
    ///
    /// Rounding up division of `a / b` is done using `(a + b - 1) / b`.
    pub(crate) fn num_shares_from_staked_amount_rounded_up(
        &self,
        amount: Balance,
    ) -> NumStakeShares {
        assert!(
            self.total_staked_balance > 0,
            "The total staked balance can't be 0"
        );
        ((U256::from(self.total_stake_shares) * U256::from(amount)
            + U256::from(self.total_staked_balance - 1))
            / U256::from(self.total_staked_balance))
        .as_u128()
    }

    /// Returns the staked amount rounded down corresponding to the given number of "stake" shares.
    pub(crate) fn staked_amount_from_num_shares_rounded_down(
        &self,
        num_shares: NumStakeShares,
    ) -> Balance {
        assert!(
            self.total_stake_shares > 0,
            "The total number of stake shares can't be 0"
        );
        (U256::from(self.total_staked_balance) * U256::from(num_shares)
            / U256::from(self.total_stake_shares))
        .as_u128()
    }

    /// Returns the staked amount rounded up corresponding to the given number of "stake" shares.
    ///
    /// Rounding up division of `a / b` is done using `(a + b - 1) / b`.
    pub(crate) fn staked_amount_from_num_shares_rounded_up(
        &self,
        num_shares: NumStakeShares,
    ) -> Balance {
        assert!(
            self.total_stake_shares > 0,
            "The total number of stake shares can't be 0"
        );
        ((U256::from(self.total_staked_balance) * U256::from(num_shares)
            + U256::from(self.total_stake_shares - 1))
            / U256::from(self.total_stake_shares))
        .as_u128()
    }

    /// Inner method to get the given account or a new default value account.
    pub(crate) fn internal_get_account(&self, account_id: &AccountId) -> Account {
        self.accounts.get(account_id).unwrap_or_default()
    }

    /// Inner method to save the given account for a given account ID.
    /// If the account balances are 0, the account is deleted instead to release storage.
    pub(crate) fn internal_save_account(&mut self, account_id: &AccountId, account: &Account) {
        if account.unstaked > 0 || account.stake_shares > 0 {
            self.accounts.insert(account_id, &account);
        } else {
            self.accounts.remove(account_id);
        }
    }
}
