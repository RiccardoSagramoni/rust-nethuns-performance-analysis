#!/bin/sh

cd performance-analysis
RUST_BACKTRACE=1 cargo run -- ../data/send_zero_rust.txt ../data/send_zero_cpp.txt ../data/send_zero "Send (zerocopy)" | tee ../data/send_zero.stats
cd -
