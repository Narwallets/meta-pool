set -e
NETWORK=testnet
OWNER=lucio.$NETWORK
MASTER_ACC=pool.$NETWORK
CONTRACT_ACC=meta-v2.$MASTER_ACC
GOV_TOKEN=token.meta.$MASTER_ACC

meta --cliconf -c $CONTRACT_ACC -acc $OWNER

export NODE_ENV=$NETWORK

## delete acc
#echo "Delete $CONTRACT_ACC? are you sure? Ctrl-C to cancel"
#read input
#near delete $CONTRACT_ACC $MASTER_ACC
#near create-account $CONTRACT_ACC --masterAccount $MASTER_ACC
#meta deploy ./res/metapool.wasm
#meta new { owner_account_id:$OWNER, treasury_account_id:treasury.$CONTRACT_ACC, operator_account_id:operator.$CONTRACT_ACC, meta_token_account_id:$GOV_TOKEN } --accountId $MASTER_ACC
# set params@meta set_params
#meta set_params
# deafult 4 pools
#meta default_pools_testnet


## redeploy code only
#meta deploy ./res/metapool.wasm  --accountId $MASTER_ACC
#meta set_params

#save this deployment  (to be able to recover state/tokens)
cp ./res/metapool.wasm ./res/metapool.$CONTRACT_ACC.`date +%F.%T`.wasm
date +%F.%T
