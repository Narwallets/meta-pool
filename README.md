# Meta Staking Pool

## Overview
What is Meta-Pool and other non-Technical Documentation

* [CLICK HERE](https://narwallets.github.io/meta-pool/)
* [Meta Pool GitBook](https://metapool.gitbook.io/master/)

## Audits
* [BlockSec Audit, Jan 17th, 2022](https://370551154-files.gitbook.io/~/files/v0/b/gitbook-x-prod.appspot.com/o/spaces%2F-MkhZe3MGAhTcvTLTzJF-887967055%2Fuploads%2FBnkI1s1NHp6rjw4vBsZo%2FBlocksec_Audit_2.pdf?alt=media&token=30a76ad3-a5ad-4454-87c0-0e0815b4170b)
* [BlockSec Audit v1.1, March 1th, 2022](https://www.metapool.app/MetaPool_BlockSec_Audit_signed_v1.1.pdf)
* [BlockSec Audit, staking wNEAR with Meta Pool on Aurora, March 20th, 2022](https://370551154-files.gitbook.io/~/files/v0/b/gitbook-x-prod.appspot.com/o/spaces%2F-MkhZe3MGAhTcvTLTzJF-887967055%2Fuploads%2FXE1zBF4pyaWCoR1zlKwW%2Fmain_signed.pdf?alt=media&token=5068e60d-2905-4d4f-a1cb-9d5fa4c607a3)

## Technical Documentation
* [Technical Notes](https://narwallets.github.io/meta-pool/technical-notes)

* [NEAR Live Contract Review (video)](https://www.youtube.com/watch?v=4gB-6yoas74)
### Repositories 

This is the Smart Contract repository. The Web App UI is at https://github.com/Narwallets/metastaking-webapp

### Change Log

#### `1.2.0` - March 2022

- All audit recommendations implemented
- set all staking pools weights in a single call


#### `1.1.0` - Dec 2021
- new fn Realize_meta_massive to auto farm $META


#### `1.0.0` - Apr 2021

- Full functionality
- Simplified user flow 
- Desk check testing https://github.com/Narwallets/sc-desk-check-parser

#### `0.1.0` - Nov 2020

- Initial version based on core-contracts/lockup and core-contracts/staking-pool
- Deposit, withdraw
- Distributed stake/unstake
- NEAR/stNEAR liquidity pool, Add/Remove liquidity
- META Minting with rewards

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


## Testing

Besides We are doing a simple ad-hoc fuzzy test for metapool.
The test generates random operations. We have a list of "invariants" the contract must satisfy to guarantee the internal accounting is consistent. We use a seeded random generator to create "operations" against the metapool (deposit, liquid-unstake, delayed-unstake, add-liquidity, remove-liquidity, end-of-epoch, compute-rewards, retrieve-funds-from-pools) in any order and amount. After each successful operation we check the contract invariants again. This is our way to tests unprepared operations combinations and make sure the internal accounting remains consistent

This is he core .rs fuzzy source https://github.com/Narwallets/meta-pool/blob/master/metapool/tests/sim/simulation_fuzzy.rs, you can navigate up from there to see what it is doing.
