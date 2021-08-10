set -e
NETWORK=testnet
OWNER=alan1.$NETWORK
MASTER_ACC=preprod-pool.$NETWORK
CONTRACT_ACC=contract4.$MASTER_ACC
TREASURY_ACC=treasury.$MASTER_ACC
OPERATOR_ACC=operator.$MASTER_ACC
GOV_TOKEN=token.$MASTER_ACC
PNEAR_TOKEN=pnear.$MASTER_ACC


export NODE_ENV=$NETWORK

## delete accout
echo "Delete $CONTRACT_ACC? are you sure? Ctrl-C to cancel"
read input

near delete $CONTRACT_ACC $MASTER_ACC
#near delete $GOV_TOKEN $MASTER_ACC
#near delete $OPERATOR_ACC $MASTER_ACC
near delete $PNEAR_TOKEN $MASTER_ACC
near create-account $CONTRACT_ACC --masterAccount $MASTER_ACC --initialBalance 5
#near create-account $GOV_TOKEN --masterAccount $MASTER_ACC
#near create-account $OPERATOR_ACC --masterAccount $MASTER_ACC
near create-account $PNEAR_TOKEN --masterAccount $MASTER_ACC --initialBalance 5
near deploy --wasmFile ./res/metapool.wasm --accountId $CONTRACT_ACC --initDeposit 1
near deploy --wasmFile ./res/pnear_token.wasm --accountId $PNEAR_TOKEN --initDeposit 1
#near deploy --wasmFile ./res/meta_token.wasm --accountId $GOV_TOKEN

##Initialize
near call $CONTRACT_ACC new '{ "owner_account_id":"'"$OWNER"'", "treasury_account_id":"'"$TREASURY_ACC"'", "operator_account_id":"'"$OPERATOR_ACC"'", "meta_token_account_id":"'"$GOV_TOKEN"'" }' --accountId $MASTER_ACC
near call pnear.preprod-pool.testnet new '{"owner_id": "preprod-pool.testnet","token_contract": "contract4.preprod-pool.testnet","min_amount_near": "1", "min_amount_token": "1","sell_only": false}' --accountId alan1.testnet

# set params@meta set_params
#meta set_params
## deafult 4 pools
#meta default_pools_testnet


## redeploy code only
#meta deploy ./res/metapool.wasm  --accountId $MASTER_ACC
#meta set_params

#save this deployment  (to be able to recover state/tokens)
cp ./res/metapool.wasm ./res/metapool.$CONTRACT_ACC.`date +%F.%T`.wasm
cp ./res/meta_token.wasm ./res/metapool.$GOV_TOKEN.`date +%F.%T`.wasm
cp ./res/pnear_token.wasm ./res/metapool.$PNEAR_TOKEN.`date +%F.%T`.wasm
date +%F.%T