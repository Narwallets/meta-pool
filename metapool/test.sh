#!/bin/bash
set -e

cd ..
bash build.sh
cd -

export RUST_BACKTRACE=1 
cargo +stable test -- --nocapture >desk-check.log

echo "-- Output sent to desk-check.log"
