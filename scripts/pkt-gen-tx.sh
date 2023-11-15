#!/bin/sh
sudo taskset -c 2 pkt-gen -i vale0:1 -f tx
