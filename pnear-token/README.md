# $pNEAR Token NEP-141 Token Contract

## Technicalities


## How to use it 

´´´bash
near call pnear.preprod-pool.testnet new '{"owner_id": "preprod-pool.testnet","total_stnear": "1", "total_tokens": "1","token_contract": "contract4.preprod-pool.testnet","min_amount_near": "1", "min_amount_token": "1","sell_only": false}' --accountId alantest.testnet
´´´

´´´bash
near call contract4.preprod-pool.testnet ft_transfer_call '{"receiver_id":"pnear.preprod-pool.testnet","amount":"5","msg":"msg"}' --accountId alantest.testnet --amount 0.000000000000000000000001
´´´
