CONTRACT_ACC=meta-pool.near
set +ex
meta-util dao propose upgrade $CONTRACT_ACC res/metapool.wasm
mkdir -p res/mainnet/metapool
cp res/metapool.wasm res/mainnet/metapool/$CONTRACT_ACC.`date +%F.%T`.wasm
date +%F.%T
