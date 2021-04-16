#!/bin/bash
set -e

RUSTFLAGS='-C link-arg=-s' cargo build --all --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/metapool.wasm res/
cp target/wasm32-unknown-unknown/release/meta_token.wasm res/
cp target/wasm32-unknown-unknown/release/staking_pool.wasm res/
cp target/wasm32-unknown-unknown/release/rewards_register.wasm res/

