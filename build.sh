#!/bin/bash
set -e

RUSTFLAGS='-C link-arg=-s' cargo build --all --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/divpool.wasm res/
cp target/wasm32-unknown-unknown/release/staking_pool.wasm res/

