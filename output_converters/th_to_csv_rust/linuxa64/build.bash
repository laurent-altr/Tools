#!/bin/bash
cd ..
cargo build --release --target aarch64-unknown-linux-gnu
cp target/aarch64-unknown-linux-gnu/release/th_to_csv ../../../exec/th_to_csv_linuxa64
