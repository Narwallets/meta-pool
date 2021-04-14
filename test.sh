#!/bin/bash
set -e

cargo +nightly build
export RUST_BACKTRACE=1 
cargo +nightly test -- --nocapture

