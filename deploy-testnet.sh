export NODE_ENV=testnet

#echo "Delete diversifying.pool.testnet? are you sure? Ctrl-C to cancel"
#read input
#near delete diversifying.pool.testnet pool.testnet --accountId pool.testnet
#near create-account diversifying.pool.testnet --masterAccount pool.testnet
#near deploy diversifying.pool.testnet ./res/diversifying_staking_pool.wasm new '{"owner_account_id":"dao.diversifying.pool.testnet", "treasury_account_id":"treasury.diversifying.pool.testnet", "operator_account_id":"lucio.testnet"}' --accountId pool.testnet

## redeploy code only
near deploy diversifying.pool.testnet ./res/diversifying_staking_pool.wasm  --accountId pool.testnet

