# Meta Staking Pool

## Overview
What is Meta-Pool and other non-Technical Documentation

[CLICK HERE](https://narwallets.github.io/meta-pool/)

## Technical Documentation
[Technical Notes](https://narwallets.github.io/meta-pool/technical-notes)

### Repositories 

This is the Smart Contract repository. The Web App UI is at https://github.com/Narwallets/metastaking-webapp

### Change Log
#### `0.1.0`

- Initial version based on core-contracts/lockup and core-contracts/staking-pool
- Deposit, withdraw
- Distributed stake/unstake
- NEAR/stNEAR liquidity pool, Add/Remove liquidity
- META Minting with rewards

#### `1.0.0`

- Full functionality
- Simplified user flow 
- Desk check testing https://github.com/Narwallets/sc-desk-check-parser

### TO DO & Help needed


#### Smart Contract  
 - [x] Deposit/withdraw
 - [x] Buy stNEAR/Stake
 - [x] Sell stNEAR/immediate unstake
 - [x] Classic unstake-wait-finish-unstake
 - [x] User trip-meter, measure rewards
 - [x] distribute staking/unstaking
 - [x] retrieve unstaked and ready
 - [x] NEAR/stNEAR Liquidity Pool, Add/Remove liquidity
 - [x] clearing mechanism on stake to restore liquidity in the NSLP
 - [x] act as a NEP-xxx MULTI-FUN-TOK (multi-token contract). Implement for NEAR, stNEAR and META
 - [ ] Dividends-pool stNEAR/META
 - [x] Staking-loans to whitelisted validators
 - [ ] Emergency Staking (from the nslp) to whitelisted validators

#### Test
 - [x] Simulation tests
 - [x] Fuzzy Testing

#### Staking pool list
 - [x] List selected staking pools, getting weight, staked & unstaked
 - [x] add a staking pool, set weight

#### Governing
 - [x] Mint and distribute META with rewards
 - [ ] Phase II - Governing DAO

#### Infrastructure
- [x] External cron to call distribute()
- [x] compute remaining epoch time
- [x] whitelist pools before adding them

#### Expansions

- [x] USDNEAR MVP: Create a collateral-based stablecoin similar to Compound's DAI, using NEAR & stNEAR as collateral
