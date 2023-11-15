#!/bin/sh
sudo taskset -c 1 perf stat -o recv/recv_rust.data ./recv/perf_recv vale0:0
