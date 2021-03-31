# Meta Staking Pool

## Overview

This contract provides the following value items for the NEAR community:

#### Helps stakers avoid putting all eggs in one basket
 This contract acts as a staking-pool that distributes it's delegated funds to several validators. By delegating to this contract, you greatly reduce the risk of getting no-rewards due to validator outages and you contribute to decentralization and censorship-resistance. Besides staking rewards, while staking you will receive $META governance tokens. (Automatic Farming) 

#### Tokenizes Stake
This contract **tokenizes, liberates, your stake** while it keeps generating staking rewards. It allows you to use it to operate on markets or [use it as collateral](https://github.com/luciotato/usdnear-stable).
#### Provides Liquid Unstaking
This contract allows users to skip the unstaking waiting period by providing a liquidity pool for *liquid unstaking*. This simplifies staking, making staking and unstaking simple and immediate.

#### Contributes to decentralization for the NEAR network
 This contract helps decentralization by distributing its delegated funds to several validators. If you own a lockup-contract with considerable funds, you can greatly contribute to the decentralization and censorship-resistance of the network and reduce your risk at the same time. Funds will be automatically distributed between several validators ensuring increased decentralization and continuous rewards.

#### Creates a Liquidity Pool and fees for Liquidity Providers
This contract includes a liquidity pool and the opportunity for liquidity providers to earn fees. The liquidity pool is a stNEAR->NEAR pool, providing the *Liquid Unstake* functionality and generating fees for the Liquidity Providers.

#### Automatic Farming
This contract integrates two NEP-141 tokens: stNEAR, *staked-NEAR* and $META, the project governance token. $METAs are farmed automatically. If you use the liquid unstake function, you'll receive $META; if you stake, you'll get $META with your staking rewards; if you are a Liquidity Provider, you'll get $META with your fees. There's also a $META multiplier: *early liquidity providers will receive more $META than late adopters*. At launch time the multipliers are:
* 5x: Liquid unstakers get 5 $META per each NEAR they pay in fees
* 10x: Stakers get 10 $META per each NEAR they receive from staking rewards
* 50x: Liquidity Providers get 50 $META per each NEAR fee generated

#### Validator Loans: a safety-net to avoid losing validators on the seat-price cliff 
* This contract will allow struggling validators to keep a seat and keep validating in case of sudden rises in seat price. There will be staking-loans available for whitelisted validators and also emergencies stakes from the liquidity pool. Pools requiring staking will have to pay x-epoch rewards in advance. Fees paid will be distributed as rewards between the stNEAR holders and/or the Liquidity providers.

## stNEAR Tokens

This contract tokenizes your stake, allowing users to manage staked near as a NEP-141 TOKEN, called **stNEAR**.

stNEARs represent staked NEAR, and can be transferred between users and sold for NEAR in the NEAR/stNEAR Liquidity Pool (paying a fee to skip the unstaking wait period). **The amount of stNEAR you hold is automatically incremented each epoch when staking rewards are paid**. This contract also includes a trip-meter functionality, so you can precisely measure rewards received.

## Liquid Unstake

Users wanting to unstake skipping the waiting period can do so in the *stNEAR->NEAR Liquidity Pool*.

In the Liquidity Pool:
 * Users providing liquidity can earn fees on each sell
 * Users wanting to unstake without the waiting period can do so for a fee.

The *stNEAR->NEAR Liquidity Pool* is a one-sided Liquidity pool. Liquidity providers add only NEAR to the liquidity pool. The pool allows other users to have "Liquid Unstakes". During a "Liquid Unstake" users sell stNEAR and take NEAR from the liquidity pool, paying a fee. The fee represents how much users value skipping the 39-52hs waiting period to receive their funds. The fee varies with the amount of NEAR in the Liquidity Pool, but the curve is capped at the extremes. Initially, discount fees are in the range 1.8%-0.25%, but the curve parameters can be adjusted by DAO governance (by the vote of $META token holders).

![example-fee-curve](images/example-fee-curve.png)

## Standard staking-pool and Lockup-Contract accounts

This contract also acts as a standard staking-pool, so users can perform classical stakes and classical unstakes.

By implementing the standard-staking-pool trait, **lockup-contract accounts** can delegate funds here, gaining risk reduction and greatly contributing to NEAR decentralization. Note: Lockup contracts can *only* perform classic stake/unstake so Lockup contracts *can not* access the liquidity pool or buy/sell stNEAR.


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

stNEAR: one stNEAR represents one staked NEAR. A stNEAR is a virtual token computed from the user’s share in the total staked funds. By staking in the Meta-pool a user mints stNEAR, by unstaking, stNEARs are burned. When staking rewards are paid, new stNEARs are minted and distributed.

--- To STAKE is to mint stNEARs ---

--- To UNSTAKE is to burn stNEAR ---

There are two ways to unstake: (from more convenient to less convenient)

1. Liquid Unstake: You unstake by selling stNEAR in the Liquidity Pool. Since you’re unstaking without waiting 39-54hs (you’re passing that waiting penalty to other users) you pay a small fee. The fee is the value of not-waiting 39-54hs. Example: You liquid-unstake 100 stNEAR for 99.8 NEAR *and* 1 $META, and you get your NEAR immediately without waiting 39-54hs. 

2. Delayed Unstake. The contract unstakes your NEAR from the staking-pools. You burn stNEAR tokens and get unstaked-near. You don’t pay a fee, but you must wait 39-54hs to move those funds to your account. Your funds remain unstated in the staking-pool for 3 or 4 epochs (39-54hs) before you can withdraw, finishing the unstake. E.g. you unstake 100 stNEAR, and you get 100 unstaked-near, 4 days later you can move your unstaked-near to your own near account.

These operations are reflected in the UI in two steps that the user must complete with 39-54hs between the two: [START Delayed Unstake] and [FINISH Delayed Unstake]

**Liquid Unstake**

In order to provide Liquid Unstake a Liquidity Pool is maintained by the contract:

* The unstaker enters the amount of stNEAR they want to unstake and the contract replies with the amount of NEAR & $META they will receive, normally with a fee between 0.25% and 1.8% depending on the liquidity pool balance and the fee curve parameters.

## Treasury
Part of the LP fees go to the DAO Treasury. Treasury funds are always stNEARs and used for:

* DAO Maintenance
* DAO Expansion
* $META holders dividends

## Governance

(When Phase II - DAO Governance is implemented)

$META holders can vote on:
* Diversification: Validator distribution list, and how much stake to distribute to each one.
* Fee curve parameters for the NEAR/stNEAR Liquidity Pool (min fee, max fee, liquidity target)
* How to use treasury funds for DAO expansion
* Operational costs fee
* Maintenance funds redirection
* Pay rewards to $META holders
* Approve validator stake-loans
* Set $META mint reward multiplier for:
  * Liquid unstakers (default 1 $META per each discounted NEAR)
  * Stakers (default 5 $META per each stNEAR of staking reward)
  * LP-providers (default 20 $META per each stNEAR fee received)

### Phase III Proposals (future)
Create a Dividends-pool as a stNEAR/$META liquidity pool allowing $META owners to burn $META to obtain stNEAR. This pool sets a base-price for $META tokens. When users vote to add stNEAR to the dividends-pool, $META base-price is incremented. Users can also vote to remove stNEAR from the dividends-pool back to the treasury, lowering the $META base price.

Users get $META tokens minted for them when:
* They do Liquid Unstakes (immediate unstaking) (default 1x multiplier)
* They receive rewards for holding stNEAR (default 5x multiplier)
* They receive fees as Liquidity Providers (default 20x multiplier)

$META governance tokens are minted and distributed to:
* users paying liquid unstaking fees
* users holding stNEAR 
* users providing liquidity.

## Operational costs

Periodic calls to `distribute_staking()/distribute_unstaking()/withdraw_from_a_pool()` are required for this contract operation. These calls consume considerable amounts of gas that is paid by the operator account. To fund this operational cost, a operator's fee percentage (0.3% by default) is taken from rewards distributions. It can be adjusted by $META governance token holders.

## Maintenance

The contract has a configurable parameter `dev_maintenance_amount`, initially 400 stNEAR, to be transferred monthly to the account `developers.near`. By DAO governance, this value can be increased and $META token holders can also re-direct up to 50% of maintenance funds to other maintainers and contributors.


## User stories:
### Alice 

Alice wants to stake her NEAR with low risk, and also help the community by promoting decentralization and censorship-resistance for the network.
Alice opens an account in the contract: meta.pool.near

Alice stakes 750,000 NEAR. Her 750,000 NEAR are distributed between the staking-pools by an automatic distribution mechanism to keep the validators balanced. She gets 750,000 stNEAR tokens. 

She starts earning staking rewards on her stNEAR, she can track precisely her rewards. She will also get $META tokens on each reward distribution.
By having stNEAR she has tokenized her stake, she can participate in other markets, and also seh can Liquid-Unstake some of her stNEAR skipping the waiting period if the need arises.

### Bob 

Bob already has an account in the meta-pool contract. He holds 10,000 stNEAR earning staking rewards. 

Bob needs to unstake 5,000 NEAR to use in an emergency. He can’t wait 4 days to get his NEAR. 

Bob Liquid-Unstakes 5,050 stNEAR and he gets 5,000 NEAR plus 250 $META (He's paying a 1% fee to get his NEAR immediately).
Bob gets NEARs in his account. Bob can use his NEAR immediately.

### Carol 

Carol is an investor. She wants to provide liquidity for the Liquid-Unstake function for a short period, earning swap fees.
Carol deposits 7,000 NEAR in the Liquidity Pool, she is the first in the pool, so she gets 7,000 shares of the Liquidity Pool.

Bob swaps 5,050 stNEAR for 5,000 NEAR and 250 $META. He pays a 1% fee to get the NEAR immediately. The Liquidity Pool delivers 5,000 NEAR to Bob and acquires 5,050 stNEAR from Bob. The new value of the Liquidity Pool is 7,050 NEAR (2,000 NEAR + 5,050 stNEAR), 

Carol share value has increased and now she owns some stNEAR via the Liquidity Pool. Carol burns all her shares and retrieves 2,000 NEAR and 5,050 stNEAR into her account. Carol has now 7,050 NEAR. Carol earned 0.7% in a few epochs.
Had her normally staked 7,000 NEAR, she would have earned only 0.1% in the same period.

### Dave

Dave is a Liquidity Provider. He wants to provide continuous liquidity for the Liquidity Pool, in order to earn a fee on each operation.

Being a Liquidity Provider can bring-in more earnings than just staking, while helping the community at the same time by providing liquid unstaking for other users.

Dave enters 100,000 NEAR to the Liquidity Pool, he gets shares of the Liquidity Pool. 

Eve swaps 50,500 stNEAR for 50,000 NEAR. She pays a 1% fee price to get her NEAR immediately.

The Liquidity Pool delivers 50,000 NEAR to Eve and acquires 50,500 stNEAR from Eve.
The Liquidity Pool has a low amount of NEAR now. After a few minutes, the liquidity pool automatically unstakes stNEAR. (Also the LP can use an internal clearing mechanism to acquire NEAR and restore liquidity automatically). After unstaking all, the pool will have liquidity restored at 100,500 NEAR.

As the Liquidity Pool operates, the NEAR amount grows, so do Dave’s share value. With each operation $META tokens are also minted for Liquidity Providers, so Dave and the other providers get $META tokens besides the fees.

-------------------------

## Future Expansions

* USDNEAR: Create a collateral-based stable coin similar to Compound's DAI, using stNEAR as collateral


-------------------------

## Technical Information, Change Log & TO-DO

See the [smart contract github repository README](https://github.com/Narwallets/meta-pool)
