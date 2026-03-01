#!/bin/bash
cd ..
cargo build --release
cp target/release/th_to_csv ../../../exec/th_to_csv_linux64
