set -e
NETWORK=testnet
OWNER=lucio.$NETWORK
MASTER_ACC=meta.pool.$NETWORK
CONTRACT_ACC=token.$MASTER_ACC

export NODE_ENV=$NETWORK

## delete acc
#echo "Delete $CONTRACT_ACC? are you sure? Ctrl-C to cancel"
#read input
#near delete $CONTRACT_ACC $MASTER_ACC
#near create-account $CONTRACT_ACC --masterAccount $MASTER_ACC
#near deploy $CONTRACT_ACC ../res/meta_token.wasm --masterAccount $MASTER_ACC
near call $CONTRACT_ACC new "{\"owner_id\":\"$OWNER\"}" --accountId $MASTER_ACC
## set params@meta set_params
#meta default_pools_testnet


## redeploy code only
#near deploy $CONTRACT_ACC ./res/meta_token.wasm --masterAccount $MASTER_ACC

#save last deployment  (to be able to recover state/tokens)
#cp ./res/meta_token.wasm ./res/meta_token.`date +%F.%T`.wasm
#date +%F.%T
