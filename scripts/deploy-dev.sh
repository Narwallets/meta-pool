set -e
NETWORK=testnet
OWNER=lucio.$NETWORK
MASTER_ACC=pool.$NETWORK
CONTRACT_ACC=test.$MASTER_ACC
GOV_TOKEN=token.$MASTER_ACC

export NODE_ENV=$NETWORK

## delete acc
echo "Delete $CONTRACT_ACC? are you sure? Ctrl-C to cancel"
read input
near delete $CONTRACT_ACC $MASTER_ACC
near create-account $CONTRACT_ACC --masterAccount $MASTER_ACC
near deploy $CONTRACT_ACC ./res/get_epoch_contract-v1.wasm
near call $CONTRACT_ACC new  --accountId $MASTER_ACC

## redeploy code only
# near deploy $CONTRACT_ACC ./res/get_epoch_contract-v1.wasm

