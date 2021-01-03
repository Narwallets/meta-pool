# Diversifying Staking Pool

## Overview
What this is? and other non-Technical Documentation

[CLICK HERE](https://narwallets.github.io/diversifying-staking-pool/)

## Technical Documentation
[Technical Notes](https://narwallets.github.io/diversifying-staking-pool/technical-notes)

### Repositories 

This is the Smart Contract repository. The Web App UI is at https://github.com/Narwallets/dapp-diversifying-staking-pool.git

### Change Log
#### `0.1.0`

- Initial version based on core-contracts/lockup and core-contracts/staking-pool
- Deposit, withdraw
- Distributed stake/unstake
- NEAR/SKASH liquidity pool, Add/Remove liquidity
- G-SKASH Minting wtih rewards

### TO DO & Help needded

#### Bounties
There are Bounties!!!! check https://github.com/Narwallets/bounties

#### Smart Contract  
 - [x] Deposit/withdraw
 - [x] Buy Skash/Stake
 - [x] Sell SKASH/immediate unstake
 - [x] Classic unstake-wait-finish-unstake
 - [x] User trip-meter, measure rewards
 - [x] distribute staking/unstaking
 - [x] retrieve unstaked and ready
 - [x] NEAR/SKASH Liquidity Pool, Add/Remove liquidity
 - [x] clearing mechanism on stake to restore liquidity in the NSLP
 - [ ] act as a NEP-xxx MULTI-FUN-TOK (multi-token with transfer-to-contract). Implement for NEAR, SKASH and G-SKASH
 - [ ] Dividends-pool SKASH/G-SKASH

#### Test
 - [ ] Unit Tests
 - [ ] Simulation tests

#### Staking pool list
 - [x] List selected staking pools, getting weight, staked & unstaked
 - [x] add a staking pool, set weight

#### Governing
 - [x] Mint and distribute G-SKASH with rewards
 - [ ] Phase II - Governing DAO

#### Infrastructure
- [ ] External chron to call distribute()
- [ ] compute remainig epoch time
- [ ] withelist pools before adding them
- [ ] auto-unstake SKASH in the NSLP (when the clearing mechanism is not enough)

