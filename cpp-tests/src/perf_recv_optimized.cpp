#include <nethuns/nethuns.h>
#include <stdio.h>
#include <getopt.h>
#include <signal.h>
#include <arpa/inet.h>
#include <net/ethernet.h>
#include <netinet/ether.h>
#include <linux/ip.h>
#include <sys/socket.h>

#include <atomic>
#include <thread>
#include <chrono>
#include <iostream>
#include <string>
#include <vector>


// nethuns socket
nethuns_socket_t* my_socket;
nethuns_socket_options netopt;
char* errbuf;

// configuration
std::string interface = "";

// stats collection
uint64_t total = 0;
#define     METER_DURATION_SECS    10 * 60 + 1
#define     METER_RATE_SECS        10


// terminate application
volatile bool term = false;

// termination signal handler
void terminate(int exit_signal)
{
    (void)exit_signal;
    term = true;
}

void terminate_program(std::chrono::system_clock::time_point stop_timestamp) {
    std::this_thread::sleep_until(stop_timestamp);
    term = true;
}

void meter() {
    auto now = std::chrono::system_clock::now();
    long old_totals = 0;
    while (!term) {
        now += std::chrono::seconds(1);
        std::this_thread::sleep_until(now);
        long x = total;
    	std::cout << "pkt/sec: " << x - old_totals << std::endl;
        old_totals = x;
    }
}


int main(int argc, char *argv[])
{
    if (argc < 2) {
        std::cerr << "Usage: " << argv[0] << " <interface>" << std::endl;
        return 1;
    }
    
    interface = argv[1];
    
    signal(SIGINT, terminate);  // register termination signal
    
    // nethuns options
    netopt =
    {
        .numblocks       = 1
    ,   .numpackets      = 4096
    ,   .packetsize      = 2048
    ,   .timeout_ms      = 0
    ,   .dir             = nethuns_in_out
    ,   .capture         = nethuns_cap_zero_copy
    ,   .mode            = nethuns_socket_rx_tx
    ,   .promisc         = false
    ,   .rxhash          = false
    ,   .tx_qdisc_bypass = true
    ,   .xdp_prog        = nullptr
    ,   .xdp_prog_sec    = nullptr
    ,   .xsk_map_name    = nullptr
    ,   .reuse_maps      = false
    ,   .pin_dir         = nullptr
    };
    
    my_socket = new nethuns_socket_t();
    errbuf = new char[NETHUNS_ERRBUF_SIZE];
    
    // setup sockets and rings
    my_socket = nethuns_open(&netopt, errbuf);
    if (!my_socket) {
        throw std::runtime_error(errbuf);
    }
    
    if (nethuns_bind(my_socket, interface.c_str(), NETHUNS_ANY_QUEUE) < 0) {
        throw nethuns_exception(my_socket);
    }
    
    // set up timer for stopping data collection after 10 minutes
    std::thread stop_th(
        terminate_program, 
        std::chrono::system_clock::now() + std::chrono::seconds(METER_DURATION_SECS)
    );
    
    // start thread for stats collection
    std::thread meter_th(meter);
    
    // case single thread (main) with generic number of sockets
    try {
        while (likely(!term)) {            
            const nethuns_pkthdr_t *pkthdr = nullptr;
            const unsigned char *frame = nullptr;
            uint64_t pkt_id = nethuns_recv(my_socket, &pkthdr, &frame);
            
            if (pkt_id == NETHUNS_ERROR) {
                throw nethuns_exception(my_socket);
            }
            
            if (pkt_id > 0) {
                // process valid packet here
                total++;
                nethuns_rx_release(my_socket, pkt_id);
            }
        }
    } catch(nethuns_exception &e) {
        if (e.sock) {
            nethuns_close(e.sock);
        }
        std::cerr << e.what() << std::endl;
        return 1;
    } catch(std::exception &e) {
        std::cerr << e.what() << std::endl;
        return 1;
    }
    
    nethuns_close(my_socket);
    return 0;
}