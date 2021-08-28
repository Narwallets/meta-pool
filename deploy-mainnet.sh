set -e
NETWORK=mainnet
SUFFIX=near
export NODE_ENV=$NETWORK

OWNER=narwallets.$SUFFIX
MASTER_ACC=meta-pool.$SUFFIX
CONTRACT_ACC=$MASTER_ACC
GOV_TOKEN=meta-token.$SUFFIX
OPERATOR_ACC=operator.meta-pool.$SUFFIX
TREASURY_ACC=treasury.meta-pool.$SUFFIX

#meta --cliconf -c $CONTRACT_ACC -acc $OWNER

# near create-account $OPERATOR_ACC --masterAccount $MASTER_ACC --accountId $OWNER --initialBalance 0.5
# near create-account $TREASURY_ACC --masterAccount $MASTER_ACC --accountId $OWNER --initialBalance 0.5

## delete acc
#echo "Delete $CONTRACT_ACC? are you sure? Ctrl-C to cancel"
#read input
#near delete $CONTRACT_ACC $MASTER_ACC
set -ex
near deploy $CONTRACT_ACC ./res/metapool.wasm \
  --initFunction "new" \
  --initArgs "{\"owner_account_id\":\"$OWNER\",\"treasury_account_id\":\"$TREASURY_ACC\",\"operator_account_id\":\"$OPERATOR_ACC\",\"meta_token_account_id\":\"$GOV_TOKEN\"}" \
  --accountId $OWNER

#meta set_params
#near call $CONTRACT_ACC set_busy "{\"value\":false}" --accountId preprod-pool.testnet --depositYocto 1
## deafult 4 pools
##meta default_pools_testnet

## redeploy code only
# near deploy $CONTRACT_ACC ./res/metapool.wasm  --accountId $MASTER_ACC
#meta set_params

#save this deployment  (to be able to recover state/tokens)
cp ./res/metapool.wasm ./res/mainnet/metapool.$CONTRACT_ACC.`date +%F.%T`.wasm
date +%F.%T
