#!/bin/sh

cd performance-analysis
RUST_BACKTRACE=1 cargo run -- ../data/recv_rust.txt ../data/recv_cpp.txt ../data/recv "Reception" | tee ../data/recv.stats
cd -
