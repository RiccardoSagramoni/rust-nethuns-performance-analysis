#!/bin/bash

HOME="~/rust-nethuns-dir/tests"
packetsize=( 64 128 256 512 )

######################################################
####  40 Gpbs EXPERIMENT
######################################################

# 1. lace -> whiskey (TX)
for s in "${packetsize[@]}"
do
	ssh whiskey "sudo taskset -c 1 pkt-gen -i enp4s0f0 -f rx > ~/pkt-gen.txt 2>&1 &"
    ssh lace "sudo taskset -c 1 timeout 13 $HOME/bin/perf_send -i enp4s0f0 -b 1024 -s $s | tee $HOME/results/40g_send_rust_$s.txt"
    ssh lace "sudo taskset -c 1 timeout 13 $HOME/bin/nethuns-perf_send -i enp4s0f0 -b 1024 -s $s | tee $HOME/results/40g_send_cpp_$s.txt"
    ssh whiskey "sudo killall pkt-gen"
done

# 2. lace <- whiskey (RX)
ssh whiskey "sudo taskset -c 1 pkt-gen -i enp4s0f0 -f tx > ~/pkt-gen.txt 2>&1 &"
ssh lace "sudo taskset -c 1 timeout 13 $HOME/bin/perf_send -i enp4s0f0 | tee $HOME/results/40g_recv_rust.txt"
ssh lace "sudo taskset -c 1 timeout 13 $HOME/bin/nethuns-perf_send -i enp4s0f0 | tee $HOME/results/40g_recv_cpp.txt"
ssh whiskey "sudo killall pkt-gen"
