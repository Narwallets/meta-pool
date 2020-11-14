# Diversified Staking Contract

## Overview

### Diversification

This contract acts as an staking-pool but distributes it's delegated funds in several staking pools. By doing that, users reduce risks of outage events form validators and also average validators fees. 

### Internal Clearing, inmmediate unstakings, ultra-low gas staking

By managing multiple users and staking pools, this contracts also gives the users the chance to **inmmediate unstakings** and **ultra-low gas stakings**. 
By acting as a clearing house users intending to stake are matched with users inteding to unstake. When matches are made, both users can complete their transacionts with ultra-low-fees ins the satke case and **immediate availability in the unstake** case.

### SKASH NEP-Tokens

This contract also allows users to treat staked near as a NEP-TOKEN, called **SKASH**.
SKASHs are staked NEARS valued 1:1 to NEAR, and can be trasnferred between contract users.

### SKASH NEP-Tokens AMM Swap

If there's no people staking and a user wants to unstake, thsi contract also offers an AMM for swapping SKASH for NEAR at discount. The user avoids the unstaking waiting period and the LP get's a fee for its service.

### Standard staking-pool

This contract also acts a a standard staking-pool, so users can unstake (with the corresponding waiting period + 1h) to convert SKASHs to NEAR.
By implementing the standard-staking-pool trait, lockup contracts can delegate funds here, gaining risk reduction and fee-averaging.


## Technical details

The contract pools all users funds an after intenal clearing matains a balanced ditribution of those funds in a list of whitelisted, low-fee, high-uptime validators.

Staking adn unstaking distribution is made during calls to heartbeat() so actual Staking adn unstaking is delayed. This delay allows the internal clearing and so the benfits of low-gas-staking and immediate unstaking.

To avoid impacting staking-pools with large unstakes, there are two constants:
```
pub const NEARS_PER_BATCH: u128 = NEAR_100K; // if amount>MAX_NEARS_SINGLE_MOVEMENT then it's splited in NEARS_PER_BATCH batches
pub const MAX_NEARS_SINGLE_MOVEMENT: u128 = NEARS_PER_BATCH + NEARS_PER_BATCH/2; //150K max movement, if you try to stake 151K, it will be split into 2 movs, 100K and 51K
```
This ensures distribution between the pools and that no pool is adversely affected by a large unstake.

## Opertional costs

Calls to heartbeat() consume gas that is mostly paid by the operator. To fund this, a owner's fee percentage (0.5% by default) of the benfits are assigned to the contract's owner. From the contract's owner fee another fee is paid to this contract's author (0.25%)

- Lock tokens until the transfers are voted to be enabled.
- Lock tokens for the lockup period and until the absolute timestamp, whichever is later.
- Lock tokens for the lockup period without a vesting schedule. All tokens will be unlocked at once once the lockup period passed.
- Lock tokens for the lockup period with a vesting schedule.
  - If the NEAR Foundation account ID is provided during initialization, the NEAR Foundation can terminate vesting schedule.
  - If the NEAR Foundation account ID is not provided, the vesting schedule can't be terminated.
- Lock tokens for the lockup period with the release duration. Tokens are linearly released on transfers are enabled.

### Guarantees

(To verify)
- The users can not lose tokens or block contract operations by using methods under staking section.
- Users owning SKASHs will accrue benefits on each epoch, except in the extreme unlikely case that all the selected validators go offline dureing that epoch.

## Change Log

### `1.0.0`

### `0.3.0`

### `0.2.0`

### `0.1.0`

- Initial version by github.com/luciotato based on core-contracts/lockup and core-contracts/staking-pool

## TO DO

### View methods

[ ] List selected staking pools, weight, staked & unstaked

### User methods

[ ] act as a NEP-21 with staked as SKASHs
[ ] act as a NEP-xxx (multi-token with trasnfer-to-contract) with staked as SKASHs
[ ] AMM with LPs and SWAP SKASHs/NEAR

### Owner's method

[ ] Set staking pool [i] account_id & weight => verify [i] is empty or current at [i] not busy|staked. whitelist new staking_pool, assign
[ ] Alter staking pool [i] weight => verify [i] is occupied & not busy. assign weight

### Test

[ ] Unit tests all new functionality
[ ] Simulation tests all new functionality

