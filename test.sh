#!/bin/bash
set -e

cargo build
cargo test -- --nocapture
