#!/bin/sh
set -ex
cargo build --target wasm32-wasi
wasm-tools component new --adapt wasi_snapshot_preview1.wasm target/wasm32-wasi/debug/reactor_tests.wasm -o component.wasm 
