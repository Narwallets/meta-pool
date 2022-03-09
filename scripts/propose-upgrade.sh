set +ex
meta-util dao propose upgrade meta-pool.near res/metapool.wasm
mkdir -p res/mainnet
cp res/metapool.wasm res/mainnet/metapool.`date +%F.%T`.wasm
date +%F.%T
