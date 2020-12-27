NETWORK=guildnet
OWNER=luciotato.$NETWORK
OPERATOR=$OWNER
MASTER_ACC=pools.$NETWORK
CONTRACT_ACC=diversifying.$MASTER_ACC

export NODE_ENV=$NETWORK

#echo "Delete $CONTRACT_ACC? are you sure? Ctrl-C to cancel"
#read input
#near delete $CONTRACT_ACC --accountId $MASTER_ACC
#near create-account $CONTRACT_ACC --masterAccount $MASTER_ACC
#near deploy $CONTRACT_ACC ./res/diversifying_staking_pool.wasm new '{"owner_account_id":"$OWNER", "treasury_account_id":"treasury.$CONTRACT_ACC", "operator_account_id":"$OPERATOR"}' --accountId $MASTER_ACC

## redeploy code only
near deploy $CONTRACT_ACC ./res/diversifying_staking_pool.wasm  --accountId $MASTER_ACC

