set -e
NETWORK=testnet
OWNER=lucio.$NETWORK
OPERATOR=$OWNER
MASTER_ACC=pool.$NETWORK
CONTRACT_ACC=diversifying.$MASTER_ACC

divy --cliconf -c $CONTRACT_ACC -acc $OWNER

export NODE_ENV=$NETWORK

## delete acc
#echo "Delete $CONTRACT_ACC? are you sure? Ctrl-C to cancel"
#read input
#near delete $CONTRACT_ACC $MASTER_ACC
#near create-account $CONTRACT_ACC --masterAccount $MASTER_ACC
#divy deploy ./res/divpool.wasm
#divy new { owner_account_id:$OWNER, treasury_account_id:treasury.$CONTRACT_ACC, operator_account_id:$OPERATOR } --accountId $MASTER_ACC
## set params
#divy set_params
#divy default_pools_testnet

## redeploy code only
divy deploy ./res/divpool.wasm  --accountId $MASTER_ACC

