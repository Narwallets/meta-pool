set -ex
NETWORK=testnet
OWNER=lucio.$NETWORK
MASTER_ACC=meta.pool.$NETWORK
CONTRACT_ACC=token.$MASTER_ACC
# token.meta.pool.testnet
export NODE_ENV=$NETWORK

## delete acc
#echo "Delete $CONTRACT_ACC? are you sure? Ctrl-C to cancel"
#read input
#near delete $CONTRACT_ACC $MASTER_ACC
#near create-account $CONTRACT_ACC --masterAccount $MASTER_ACC
near deploy $CONTRACT_ACC ../res/meta_token.wasm --masterAccount $MASTER_ACC
#near call $CONTRACT_ACC new "{\"owner_id\":\"$OWNER\"}" --accountId $MASTER_ACC

set -e
#save last deployment  (to be able to recover state/tokens)
cp ../res/meta_token.wasm ../res/testnet/meta_token.`date +%F.%T`.wasm
#date +%F.%T
