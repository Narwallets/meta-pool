#!/bin/bash
set -e

RUSTFLAGS='-C link-arg=-s' cargo +stable build --all --target wasm32-unknown-unknown --release
cp -u target/wasm32-unknown-unknown/release/metapool.wasm res/
cp -u target/wasm32-unknown-unknown/release/meta_token.wasm res/
cp -u target/wasm32-unknown-unknown/release/staking_pool.wasm res/
cp -u target/wasm32-unknown-unknown/release/get_epoch_contract.wasm res/

