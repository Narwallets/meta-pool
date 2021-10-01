set -e

NETWORK=mainnet
SUFFIX=near
export NODE_ENV=$NETWORK

OWNER=narwallets.$SUFFIX
MASTER_ACC=meta-token.$SUFFIX
CONTRACT_ACC=$MASTER_ACC

METAPOOL_CONTRACT=meta-pool.$SUFFIX

## delete acc
#echo "Delete $CONTRACT_ACC? are you sure? Ctrl-C to cancel"
#read input
#near delete $CONTRACT_ACC $MASTER_ACC
#near create-account $CONTRACT_ACC --masterAccount $MASTER_ACC
near deploy $CONTRACT_ACC ../res/meta_token.wasm --masterAccount $MASTER_ACC
#near call $CONTRACT_ACC new "{\"owner_id\":\"$OWNER\"}" --accountId $MASTER_ACC
#near call $CONTRACT_ACC add_minter "{\"account_id\":\"$METAPOOL_CONTRACT\"}" --accountId $OWNER --depositYocto 1


## redeploy code only
#near deploy $CONTRACT_ACC ./res/meta_token.wasm --masterAccount $MASTER_ACC

#save last deployment  (to be able to recover state/tokens)
cp ../res/meta_token.wasm ../res/meta_token.`date +%F.%T`.wasm
date +%F.%T
