#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release -j
cargo build --release --manifest-path rust/Cargo.toml -p smart_hedge_cli
cargo test --workspace --manifest-path rust/Cargo.toml
SMART_HEDGE_CORE=build/smart_dynamic_hedge ./rust/target/release/smart-hedge --config config.example.json once --symbol SPY
