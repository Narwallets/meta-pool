set -e

NETWORK=mainnet
export NEAR_ENV=$NETWORK

SUFFIX=near
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
mkdir -p ../res/mainnet/meta-token
cp ../res/meta_token.wasm ../res/mainnet/meta-token/$CONTRACT_ACC.`date +%F.%T`.wasm
date +%F.%T
