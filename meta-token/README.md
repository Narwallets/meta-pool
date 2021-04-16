# $META Token NEP-141 Token Contract

## Technicalities

This is a NEP-141 Standard Token Contract plus:

* The meta-pool contract has the ability to mint tokens here
* $META tokens are "virtual" in the meta-pool contract and can be "harvested" by minting here NEP-141 tokens
* A separate contract is needed so the users can see $META in their wallets and use $META in any NEP-141 compatible DEFI app
* The meta-pool itself is the NEP-141 contract for stNEAR, and to avoid introducing the complexity of multi-fungible-tokens on a single contract, it's better to facilitate DEFI integration by having a single NEP-141 contract for each token
* this contract will be deployed at token.meta.pool.(near|testnet)


