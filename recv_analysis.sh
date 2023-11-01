#!/bin/sh

cd performance-analysis
cargo run -- ../data/recv_rust.txt ../data/recv_cpp.txt ../data/recv_out.svg
cd -
