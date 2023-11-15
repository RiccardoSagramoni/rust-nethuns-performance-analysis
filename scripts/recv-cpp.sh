#!/bin/sh
sudo taskset -c 1 perf stat -o recv/recv_cpp.data ./recv/nethuns-perf_recv vale0:0
