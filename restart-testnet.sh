export NODE_ENV=testnet
near delete meta.pool.testnet asimov.testnet
near create-account meta.pool.testnet --masterAccount pool.testnet
. deploy-testnet.sh
near call meta.pool.testnet new '{"owner_account_id":"dao.meta.pool.testnet", "treasury_account_id":"treasury.meta.pool.testnet", "operator_account_id":"lucio.testnet"}' --accountId pool.testnet
