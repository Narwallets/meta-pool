#!/bin/bash
set -e

cargo +nightly build
cargo +nightly test -- --nocapture

