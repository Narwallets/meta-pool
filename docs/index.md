# Meta Staking Pool

## Overview

This contract provides the following value items for the NEAR community:

#### Helps stakers avoid putting all eggs in one basket
* This contract acts as a staking-pool that distributes it's delegated funds to several validators. By delegating to this contract, you greatly reduce the risk of getting no-rewards due to a validators' outage and you contribute to decentralization of the network. Besides rewards, by staking you wil receive governance tokens: $META

#### Provides immediate unstake 
* This contract allows users to skip the waiting period after unstaking by providing a liquidity pool for inmediate unstaking. This also creates the opportunity for liquidity providers to earn fees for this service. 

#### Contributes to decentralization for the NEAR network
* This contract helps decentralization by distributing it's delegated funds to several validators. If you own a lockup-contract with considerable funds, you can greatly contribute to the decentralization of the network and reduce your risk. Your funds will be automatically distributed between several validators ensuring increased decentralization and continuous rewards.

#### Creates new Liquidity Pools
* This contract includes several liquidity pools and the opportunity for liquidity providers to earn fees. The main pool is the NEAR/stNEAR pool that provides immediate unstake (sell stNEAR) for a fee 0.5-10%. There will be other pools like the stNEAR/$META for the the governance tokens.

#### Creates a safety-net to avoid losing validators on the seat-price cliff 
* This contract will allow struggling validators to keep a seat and keep validating in case of sudden rises in seat-price. There will be staking-loans available for whitelisted validators and also emergencys stakes from the liquidity pool. Pools requiring staking will have to pay 8-epoch rewards in advance. All fees wil be distributed as rewards between the stNEAR holders or the Liquidity providers.

## stNEAR Tokens

This contract allows users to manage staked near as a TOKEN, called **stNEAR**.

stNEARs repesent staked NEAR, and can be transferred between users and sold for NEAR in the NEAR/stNEAR Liquidity Pool (paying a fee to skip the unstaking wait period). The amount of stNEAR you hold is automatically incremented each epoch when staking rewards are paid. This contract also includes a trip-meter functionality, so you can preciseliy measure rewards received.

## Immediate Unstakings

Users wanting to unstake skipping the waiting period can do so in the *NEAR/stNEAR Liquidity Pool*.

In the Liquidity Pool:
 * Users providing liquidity can earn fees on each sell
 * Users wanting to unstake without the waiting period can do so for a fee

The *NEAR/stNEAR Liquidity Pool* is a one-sided Liquidty pool. Liquidity providers add only NEAR to the Liq. pool. The Liq. pool allows other users to SELL stNEAR for NEAR (unstake) at a discounted price. The discount represents how much users value skipping the 39-52hs waiting period to receive their funds. The discount varies with the amount of NEAR in the Liquidity Pool, but the curve is capped at the extremes. By default, discount fees are in the range 0.5-5%, but the curve parameters can be adjusted by DAO governance (by the vote of $META governance token holders).

![example-fee-curve](images/example-fee-curve.png)

## Standard staking-pool

This contract also acts as a standard staking-pool, so users can perform classical stakes and classical unstakes (but with the possibility of extra waiting time because delayed unstake and/or unstaking congestion).

## Lockup contracts

By implementing the standard-staking-pool trait, *lockup contracts* can delegate funds here, gaining risk reduction and greately contributing to NEAR decentralization. Lockup contracts can only perform classic stake/unstake so Lockup contracts *can not* access the liquidity pools to sell stNEAR.

## Decentralization

This contract helps the community by increasing decentralization, spliting stake automatically betweeen several validators, and also rescuing validators falling from the seat-price cliff.


## Technical details

The contract pools all users' funds and keeps a balanced distribution of those funds in a list of whitelisted, low-fee, high-uptime validators.

Staking and unstaking distribution is done by periodically calling `distribute_staking()/distribute_unstaking()`, so actual staking and unstaking are delayed. 

Users can choose to "sell" some of their stNEAR to Liquidity Providers for a fee. Liquidity Providers get the stNEAR+fee and deliver NEAR. No stake/unstake is performed at that point.

### Guarantees

(To verify)
- The users can not lose tokens or block contract operations by using methods under staking section.
- Users owning stNEARs will accrue rewards on each epoch, except in the extreme unlikely case that ALL validators go offline during that epoch.

## Use Cases

Definitions:

stNEAR: one stNEAR represents one staked NEAR. A stNEAR is a virtual token computed from the user’s share in the total staked funds. By staking in the meta pool a user mints stNEAR, by unstaking, stNEARs are burned. When staking rewards are paid, new stNEARs are minted and distributed.

--- To BUY stNEAR is equivalent to STAKE  ---

--- To SELL stNEAR is equivalent to UNSTAKE without the waiting period ---

**To buy stNEAR and to stake are the same operation for the user.**

In order to stake the user buys stNEAR tokens. Buy stNEAR/Stake are the same operation. When buying stNEAR the price is always fixed: 1 NEAR = 1 stNEAR

**To sell stNEAR and to un-stake are similar.**

There are two ways to un-stake: (from more convenient to less convenient)

1. Sell stNEAR at a discount price. You un-stake by selling stNEAR (staked NEAR). Since you’re unstaking without waiting 39-54hs (you’re passing that waiting penalty to other users) you get a discounted price. The discount is the value you place on not-waiting 39-54hs. E.g. you sell 100 stNEAR (unstake) for 99 NEAR and get the near immediately without waiting 39-54hs.


2. Classical unstake. The contract unstakes your NEAR from the staking-pools. You burn stNEAR tokens and get unstaked-near. You don’t get a discounted price, but you must wait 39-54hs to move those funds to your account. Your funds remain unstaked in the staking-pool for 3 or 4 epochs (39-54hs) before you can withdraw finishig the unstake. E.g. you unstake 100 stNEAR, and you get 100 unstaked-near, 4 days later you can move your unstaked-near to your “available” balance and then withdraw to your own near account.

This operations are reflected in the UI in two steps that the user must complete with 39-54hs between the two: [START UNSTAKE] and [FINISH UNSTAKE]

**Sell stNEAR**

In order to provide immediate unstake (sell stNEAR) a Liquidity Pool and a SELL stNEAR mechanism are provided by the contract:

* TO SELL stNEAR: The seller enters the amount of stNEAR they want to sell and the contract replies with the amount of NEAR they will receive, normally with a discount 1%-5%, depending on the liquidity pool NEAR balance and the fee curve parameters.


## Treasury
Part of the NEAR/stNEAR LP fees goes to the DAO Treasury. Treasury funds are always stNEARs and used for:

* DAO Maintenance
* DAO Expansion
* $META holders dividends

## Maintenance

The contract has a configurable parameter `dev_maintenance_amount`, initially 2500 stNEAR, to be transferred monthly to the account `developers.near`. By DAO governance, this value can be increased and $META token holders can also re-direct up to 50% of maintenance funds to other maintainers and contributors.

## Governance

(When Phase II - DAO Governance is implemented)

$META holders can vote on:
* Diversification: Validator distribution list, and how much NEAR to distribute to each one.
* Fee curve parameters for the NEAR/stNEAR Liquidity Pool (min fee, max fee, slope)
* How to use treasury funds for DAO expansion
* Operational costs fee
* Maintenance funds redirections
* Move treasury funds in/out of the $META dividends-pool
* $META mint reward multiplier for:
  * stNEAR-sellers/immediate unstake (default 1 $META per each discounted NEAR)
  * Stakers (default 5 $META per each stNEAR of staking reward)
  * LP-providers (default 20 $META per each stNEAR fee received)
* Approve stake-loans to struggling validators

Half of treasury funds must be used for DAO expansion and maintenance. The other 50% can be user for expansion by presenting proposals, or can be moved to the dividends-pool (once a month). 

The Dividends-pool is a stNEAR/$META liquidity pool allowing $META owners to burn $META to obtain stNEAR. This pool sets a base-price for $META tokens. When users vote to add stNEAR to the dividends-pool, $META base-price is incremented. Users can also vote to remove stNEAR from the dividends-pool back to the treasury, lowering the $META base price.

Users get $META tokens minted for them when:
* They sell stNEAR (immediate unstaking) (default 1x multiplier)
* They receive rewards for holding stNEAR (default 5x multiplier)
* They receive fees in the NEAR/stNEAR Liquidity pool (default 20x multiplier)

$META governance tokens are minted and distributed to:
* users holding stNEAR and 
* users providing liquidity.
* users paying immediate unstaking fees

## Operational costs

Periodic calls to `distribute_staking()/distribute_unstaking()/withdraw_from_a_pool()` are required for this contract operation. This calls consume considerable amounts of gas that is paid by the operator account. To fund this operational cost, a operator's fee percentage (0.3% by default) is taken from rewards distributions. It can be adjusted by $META governance token holders.


## User stories:
### Alice
Alice wants to stake her NEAR with low risk, and also help the community by promoting validators diversification. 
Alice opens an account in the contract: meta.pool.near

Alice deposits 750_000 NEAR in her div-pool account. 
Alice buys 750_000 stNEAR. Her 750_000 NEAR are distributed between the staking-pools by an automatic distribution mechanism to keep the validators balanced. 

She starts earning staking rewards on her stNEAR, she can track precisely her rewards. She will also get $META tokens.
By holding stNEAR she has the possibility to sell some of her stNEAR skipping the waiting period if the need arises.

### Bob
Bob already has an account in the meta-pool contract. He has 10_000 stNEAR earning rewards. 

Bob needs to unstake 5_000 NEAR to use in an emergency. He can’t wait 39-54hs to get his NEAR. 

Bob sells 5_050 stNEAR for 5_000 NEAR. He sells at a 1% discounted price to get the NEAR immediately.
Bob gets the NEAR in his div-pool account. 
Bob can use his NEAR immediately.

### Carol
Carol is an investor. She wants to provide liquidity for the NEAR/stNEAR pool for a short period, earning operation fees.
Carol deposits 7_000 NEAR in her div-pool account
Carol adds her 7_000 NEAR to the NEAR/stNEAR liquidity pool, she is the first in the pool, so she gets 7_000 shares of the N/S-liq-pool.

Bob swaps 5_050 stNEAR for 5_000 NEAR. He sells at a 1% discounted price to get the NEAR immediately. The N/S-liq-pool delivers 5_000 NEAR to Bob and acquires 5_050 stNEAR from Bob. The new value of the N/S-liq-pool is 7_050 NEAR (2000 NEAR+5050 stNEAR), 

Carol shares value have increased, and now she owns some stNEAR via the N/S-liq-pool. Carol burns all her shares and retieves 2_000 NEAR and 5_050 stNEAR into her account. Carol has now 7_050 NEAR. Carol earned 0.7% in a few epochs.
Had her normaly staked 7_000 NEAR, she would have earned only 0.1% 

### Dave
Dave is a Liquidity Provider. He wants to provide continuous liquidity for the stNEAR/NEAR pool, in order to earn a fee on each operation.

Being a Liquidity Provider can bring-in more earnings than just staking, while helping the community at the same time by providing immediate unstaking for other users, and also helping decentralization by providing emregency stakings.

Dave enters 100_000 NEAR to the NEAR/stNEAR liquidity pool (nslp), he gets shares of the N/S-liq-pool. 

Eve swaps 50_500 stNEAR for 50_000 NEAR. She sells at a 1% discounted price to get the NEAR immediately

The N/S-liq-pool delivers 50_000 NEAR to Eve and acquires 50_500 stNEAR from Eve.
The liquidity pool has now a low amount of NEAR now. After a few minutes, the liquidity pool automatically unstakes stNEAR. The LP can use a clearing mechanism to acquire NEAR and restore liquidity automatically. After unstaking all, the pool will have 100_500 NEAR.

As the N/S-liq-pool operates, the NEAR amount grows, as Dave’s nslp-shares value. With each operation $META tokens are also minted, and Dave and the other providers get $META tokens besides the fees.

-------------------------

## Future Expansions

* USDN: Create a collateral-based stablecoin similar to Compound's DAI, using NEAR & stNEAR as collateral


-------------------------

## Technical Information, Change Log & TO-DO

See the [smart contract github repository README](https://github.com/Narwallets/meta-pool)

