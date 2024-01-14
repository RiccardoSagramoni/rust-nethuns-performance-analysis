#!/bin/sh

cd performance-analysis
RUST_BACKTRACE=1 cargo run -- ../data/send_rust.txt ../data/send_cpp.txt ../data/send "Transmission" | tee ../data/send.stats
cd -
