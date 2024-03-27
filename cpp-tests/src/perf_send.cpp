#include <nethuns/nethuns.h>
#include <stdio.h>
#include <getopt.h>
#include <signal.h>

#include <atomic>
#include <thread>
#include <chrono>
#include <iostream>
#include <string>
#include <vector>


// help message for command line usage
const std::string help_brief = "Usage:  nethuns-send [ options ]\n" \
								"Use --help (or -h) to see full option list and a complete description.\n\n"
								"Required options: \n" \
								"\t\t\t[ -i <ifname> ] \t set network interface \n" \
								"Other options: \n" \
								"\t\t\t[ -b <batch_sz> ] \t set batch size \n" \
								"\t\t\t[ -s ] \t\t\t set packet size \n";
								

// nethuns socket
nethuns_socket_t* out = new nethuns_socket_t();
struct nethuns_socket_options netopt;
char* errbufs = new char[NETHUNS_ERRBUF_SIZE];

// configuration
uint64_t pktid = 0;
std::string interface = "";
int batch_size = 1;
unsigned int packetsize = 64;
#define PACKET_HEADER_LEN    30

// stats collection
std::atomic<uint64_t> total(0);
#define     METER_DURATION_SECS    10 * 60 + 1
#define     METER_RATE_SECS        10


// terminate application
std::atomic<bool> term(false);

// termination signal handler
void terminate(int exit_signal)
{
	(void)exit_signal;
	term.store(true, std::memory_order_relaxed);
}

void terminate_program(std::chrono::system_clock::time_point stop_timestamp) {
	std::this_thread::sleep_until(stop_timestamp);
	term.store(true, std::memory_order_relaxed);
}

void meter() {
	auto time = std::chrono::system_clock::now();
	uint64_t old_total = 0;
	
	while (!term.load(std::memory_order_relaxed)) {
		time += std::chrono::seconds(METER_RATE_SECS);
		std::this_thread::sleep_until(time);
		
		uint64_t new_total = total.load(std::memory_order_acquire);
		if (new_total < old_total) {
			// overflow detected
			exit(1);
		}
		
		std::cout << new_total - old_total << std::endl;
		old_total = new_total;
	}
}


void parse_command_line(int argc, char *argv[]);


inline std::chrono::system_clock::time_point next_meter_log() {
	return std::chrono::system_clock::now() + std::chrono::seconds(METER_RATE_SECS);
}


int main(int argc, char *argv[])
{
	parse_command_line(argc, argv);
	
	signal(SIGINT, terminate);  // register termination signal
	
	// nethuns options
	netopt = {
		.numblocks       = 1
	,   .numpackets      = 1024
	,   .packetsize      = 0
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
	
	// Generate packet
	const int payload_len = packetsize - PACKET_HEADER_LEN;
	unsigned char* payload = new unsigned char[payload_len];
	memset(payload, 0xFF, packetsize - PACKET_HEADER_LEN);
	
	// Init nethuns socket
	out = nethuns_open(&netopt, errbufs);
	if (!out) {
		std::cerr << "Failed to open socket" << std::endl;
		return 1;
	}
	
	if (nethuns_bind(out, interface.c_str(), NETHUNS_ANY_QUEUE) < 0) {
		std::cerr << "Failed to bind socket to interface " << interface << std::endl;
		return 1;
	}
	
	// set up timer for stopping data collection after 10 minutes
	std::thread stop_th(
		terminate_program, 
		std::chrono::system_clock::now() + std::chrono::seconds(METER_DURATION_SECS)
	);
	
	std::thread meter_th(meter);
	
	try {
		uint64_t local_total = 0;
		
		while (!term.load(std::memory_order_relaxed)) {     
			// Prepare batch
			for (int n = 0; n < batch_size; n++) {
				if (nethuns_send(out, payload, payload_len) <= 0) {
					break;
				}
				local_total++;
				if (local_total & 0x3FF) { // update every 1024 packets
					total.store(local_total, std::memory_order_release);
				}
			}
			
			// Send batch
			nethuns_flush(out);
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
	
	nethuns_close(out);
	delete payload;
	return 0;
}


void parse_command_line(int argc, char *argv[])
{
	// parse options from command line
	int opt = 0;
	int optidx = 0;
	opterr = 1;     // turn on/off getopt error messages
	if (argc > 1 && argc < 10) {
		while ((opt = getopt_long(argc, argv, "hi:b:s:", NULL, &optidx)) != -1) {
			switch (opt) {
			case 'h':
				std::cout << help_brief << std::endl;
				std::exit(0);
				return;
			case 'i':
				if (optarg)
					interface = optarg;
				break;
			case 'b':
				if (optarg)
					batch_size = atoi(optarg);
				break;
			case 's':
				if (optarg)
					packetsize = atoi(optarg);
				break;
			default:
				std::cerr << "Error in parsing command line options.\n" << help_brief << std::endl;
				std::exit(-1);
				return;
			}
		}
	} else {
		std::cerr << help_brief << std::endl;
		std::exit(-1);
		return;
	}
	
	if (packetsize <= PACKET_HEADER_LEN) {
		std::cerr << "Packet size must be greater than " << PACKET_HEADER_LEN << std::endl;
		std::exit(-1);
		return;
	}

	std::cout << "\nTest " << argv[0] << " started with parameters \n"
			  << "* interface: " << interface << " \n"
			  << "* batch_size: " << batch_size << " \n"
			  << "* packetsize: " << packetsize << " \n"
			  << std::endl;
}
