#!/bin/bash
set -e

bash build.sh
export RUST_BACKTRACE=1 
cargo +nightly test -- --nocapture

