#!/bin/sh
sudo taskset -c 1 perf stat -o send/send_zero_cpp.data ./send/nethuns-perf_send -i vale0:0 -b 64 -z
