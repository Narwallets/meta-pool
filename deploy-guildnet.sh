set -e
NETWORK=guildnet
OWNER=luciotato.$NETWORK
OPERATOR=$OWNER
MASTER_ACC=pools.$NETWORK
CONTRACT_ACC=meta.$MASTER_ACC

divy --cliconf -c $CONTRACT_ACC -acc $OWNER

export NODE_ENV=$NETWORK

#echo "Delete $CONTRACT_ACC? are you sure? Ctrl-C to cancel"
#read input
#near delete $CONTRACT_ACC $MASTER_ACC
#near create-account $CONTRACT_ACC --masterAccount $MASTER_ACC
#divy deploy ./res/meta_staking_pool.wasm
#divy new { owner_account_id:$OWNER, treasury_account_id:treasury.$CONTRACT_ACC, operator_account_id:$OPERATOR } --accountId $MASTER_ACC

## redeploy code only
divy deploy ./res/meta_staking_pool.wasm  --accountId $MASTER_ACC

