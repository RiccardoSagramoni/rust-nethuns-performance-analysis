#!/bin/sh

cd performance-analysis
cargo run -- ../data/send_rust.txt ../data/send_cpp.txt ../data/send_out.svg
cd -
