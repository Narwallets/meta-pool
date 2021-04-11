#!/bin/bash
set -e

cargo +nightly build
cargo +nightly test -- --nocapture >desk-check.log

echo "-- Output sent to desk-check.log"
