# Diversified Staking Contract

This is the Smart Contract. The Web DApp UX is at https://github.com/Narwallets/dapp-diversifying-staking-pool.git

## Overview

### Diversification

This contract acts as an staking-pool but distributes it's delegated funds in several staking pools. By delegating to this contract, users greatly reduce the risk of getting no-rewards because validators' outage events and also get averaged validators' fees. 

### Internal Clearing, inmmediate unstakings, ultra-low gas staking

By managing multiple users and staking pools, this contracts also gives the users the chance to **inmmediate unstakings** and **ultra-low gas stakings**. 
By acting as a clearing house, users intending to stake are matched with users inteding to unstake; when matches are made, both users can complete their transactions with ultra-low-fees in the stake case and **immediate availability in the unstake** case.

### SKASH NEP-Tokens

This contract also allows users to treat staked near as a NEP-TOKEN, called **SKASH**.
SKASHs are staked NEARS valued 1:1 to NEAR, and can be trasnferred between contract users and swapped with NEAR (discounting unstaking wait period).

### SKASH NEP-Tokens AMM Swap

If there's no users staking but a user wants an *immediate unstake*, this contract also offers an AMM for swapping SKASH for NEAR at discount. The user avoids the unstaking waiting period and the LP get's a fee for its service.

### Standard staking-pool

This contract also acts a a standard staking-pool, so users can unstake (with the corresponding waiting period + 1h for the diversification mechanism).

By implementing the standard-staking-pool trait, lockup contracts can delegate funds here, gaining risk reduction and fee-averaging. 

This contract also helps the community by increasing decentralization, spliting large sums automatically betweeen several validators.


## Technical details

The contract pools all users' funds, and after an internal clearing, mantains a balanced distribution of those funds in a list of whitelisted, low-fee, high-uptime validators.

Staking and unstaking distribution is made during calls to `heartbeat()` so actual staking and unstaking is delayed. This delay allows the internal clearing and the benfits of *ultra-low-gas-staking and immediate unstaking*.

To avoid impacting staking-pools with large unstakes, this contract has a maximum movement amount during heartbeat (this is transparent to users):

```
const NEARS_PER_BATCH: u128 = NEAR_100K; 
// if amount>MAX_NEARS_SINGLE_MOVEMENT then it's splited in NEARS_PER_BATCH batches

const MAX_NEARS_SINGLE_MOVEMENT: u128 = NEARS_PER_BATCH + NEARS_PER_BATCH/2;
//150K max movement, if you try to stake 151K, it will be split into 2 movs, 100K and 51K

```

This maximum ensures a good distribution of large sums between the pools and that no pool is adversely affected by a large unstake.

## Operational costs

Periodic calls to `heartbeat()` are required for this contract opertion. This consumes gas that is mostly paid by the operator. To fund this operational cost, a owner's fee percentage (0.5% by default) is assigned to the contract's owner. From the contract's owner fee another fee is paid to the contract's authors (0.25% by default)


### Guarantees

(To verify)
- The users can not lose tokens or block contract operations by using methods under staking section.
- Users owning SKASHs will accrue rewards on each epoch, except in the extreme unlikely case that all the selected validators go offline dureing that epoch.

## Change Log

### `0.2.0`

- TO DO

### `0.1.0`

- Initial version by github.com/luciotato based on core-contracts/lockup and core-contracts/staking-pool

## TO DO

TODO & Issues # 

### View methods

[ ] List selected staking pools, getting weight, staked & unstaked

### User methods

[ ] act as a NEP-21 with staked as SKASHs

[ ] act as a NEP-xxx (multi-token with trasnfer-to-contract) with staked as SKASHs

[ ] AMM with LPs and SWAP SKASHs/NEAR

### Owner's method

[ ] Set staking pool [i] account_id & weight => verify [i] is empty or current at 
[i] not busy|staked. whitelist new staking_pool, assign

[ ] Alter staking pool [i] weight => verify [i] is occupied & not busy. assign weight

### Test

[ ] Unit tests for all the new functionality

[ ] Simulation tests for all new functionality

