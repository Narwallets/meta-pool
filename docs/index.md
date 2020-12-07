# Diversifying Staking Pool

This is the Smart Contract. The Web DApp UX is at https://github.com/Narwallets/dapp-diversifying-staking-pool.git

## Overview

### Diversification

This contract acts as an staking-pool but distributes it's delegated funds in several validators. By delegating to this contract, users greatly reduce the risk of getting no-rewards because validators' outage events and also get averaged validators' fees. 

### SKASH NEP-Tokens

This contract also allows users to treat staked near as a NEP-TOKEN, called **SKASH**.
SKASHs are staked NEARS valued 1:1 to NEAR, and can be transferred between contract users and swapped with NEAR in the NEAR/SKASH Liquidity Pool (discounting unstaking wait period).

### Liquid Unstakings, Staking at Discounted Price

By managing multiple users and staking pools, this contracts gives the users the chance to **inmmediate unstakings** and **discounted-price stakings**. 
Users intending to stake and Liquidity Providers are matched with users intending to unstake in the *NEAR/SKASH Liquidity Pool*.

In the Liquidity Pool:
 * Users providing liquidity can earn fees and stake at a discounted price
 * Users wanting to unstake without the waiting period can do so for a fee

### Standard staking-pool

This contract also acts a a standard staking-pool, so users can unstake (with the corresponding waiting period + 1h for the diversification mechanism).

By implementing the standard-staking-pool trait, lockup contracts can delegate funds here, gaining risk reduction and fee-averaging. 

This contract also helps the community by increasing decentralization, spliting large sums automatically betweeen several validators.


## Technical details

The contract pools all users' funds and mantains a balanced distribution of those funds in a list of whitelisted, low-fee, high-uptime validators.

Staking and unstaking distribution is made during calls to `heartbeat()` so actual staking and unstaking are delayed. 

To avoid impacting staking-pools with large unstakes, this contract has a maximum movement amount during heartbeat (this is transparent to users):

```
const NEARS_PER_BATCH: u128 = NEAR_100K; 
// if amount>MAX_NEARS_SINGLE_MOVEMENT then it's splited in NEARS_PER_BATCH batches

const MAX_NEARS_SINGLE_MOVEMENT: u128 = NEARS_PER_BATCH + NEARS_PER_BATCH/2;
//150K max movement, if you try to stake 151K, it will be split into 2 movs, 100K and 51K

```

This maximum ensures a good distribution of large sums between the pools, and that no pool is adversely affected by a large unstake.

## Operational costs

Periodic calls to `heartbeat()` are required for this contract opertion. This consumes gas that is mostly paid by the operator. To fund this operational cost, a owner's fee percentage (0.5% by default) is assigned to the contract's owner. From the contract's owner fee another fee is paid to the contract's developers (0.2% by default)


### Guarantees

(To verify)
- The users can not lose tokens or block contract operations by using methods under staking section.
- Users owning SKASHs will accrue rewards on each epoch, except in the extreme unlikely case that all the selected validators go offline during that epoch.

## Technical Information, Change Log & TO-DO

See the [github repository](https://github.com/Narwallets/diversifying-staking-pool)

