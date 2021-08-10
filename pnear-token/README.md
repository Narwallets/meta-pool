# $pNEAR Token NEP-141 Token Contract

* Deposit $stNEAR and receive $pNEAR
* $pNEAR = $NEAR

## Technicalities


## How to use it 

Initialize
´´´bash
near call pnear.preprod-pool.testnet new '{"owner_id": "preprod-pool.testnet","token_contract": "contract4.preprod-pool.testnet","sell_only": false}' --accountId alan1.testnet
´´´

Stake in meta-pool
´´´bash
near call contract4.preprod-pool.testnet deposit_and_stake --accountId alan1.testnet --amount 15  
´´´

Transfer stNEAR to pnear-token contract.
´´´bash
near call contract4.preprod-pool.testnet ft_transfer_call '{"receiver_id":"pnear.preprod-pool.testnet","amount":"5","msg":"msg"}' --accountId alan1.testnet --depositYocto 1 --gas 100000000000000
´´´
