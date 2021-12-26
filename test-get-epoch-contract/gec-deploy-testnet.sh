set -e
NETWORK=testnet
OWNER=lucio.$NETWORK
MASTER_ACC=pool.$NETWORK
OPERATOR_ACC_SUFFIX=.meta.pool.testnet
CONTRACT_ACC=get-epoch.$MASTER_ACC

export NEAR_ENV=$NETWORK
#near create-account $CONTRACT_ACC --masterAccount $MASTER_ACC
near deploy $CONTRACT_ACC ./res/get_epoch_contract.wasm  --accountId $MASTER_ACC --networkId $NETWORK
#near call $CONTRACT_ACC new --accountId $MASTER_ACC
date
