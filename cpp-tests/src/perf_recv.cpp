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


// help message for command line usage
const std::string help_brief = "Usage:  nethuns-send [ options ]\n" \
								"Use --help (or -h) to see full option list and a complete description.\n\n"
								"Required options: \n" \
								"\t\t\t[ -i <ifname> ] \t set network interface \n" \
								"Other options: \n" \
								"\t\t\t[ -n ] \t\t\t set number of packets \n" \
								"\t\t\t[ -s ] \t\t\t set packet size \n";


// nethuns socket
nethuns_socket_t* my_socket;
nethuns_socket_options netopt;
char* errbuf;

// configuration
std::string interface = "";
unsigned int numpackets = 1024;
unsigned int packetsize = 0;

// stats collection
std::vector<uint64_t> collected_totals;
std::atomic<bool> logging(false);
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


void parse_command_line(int argc, char *argv[]);


inline std::chrono::system_clock::time_point next_meter_log()
{
	return std::chrono::system_clock::now() + std::chrono::seconds(METER_RATE_SECS);
}


void logging_timer() {
	while (!term.load(std::memory_order_relaxed)) {
		std::this_thread::sleep_for(std::chrono::seconds(METER_DURATION_SECS));
		logging.store(true, std::memory_order_release);
	}
}


void execute ()
{
	uint64_t total = 0;
	
	const nethuns_pkthdr_t *pkthdr = nullptr;
	const unsigned char *frame = nullptr;
	
	std::thread logging_timer_thread(logging_timer);
	
	while (!term.load(std::memory_order_relaxed)) {       
		// Print logging stats
		if (logging.load(std::memory_order_acquire)) {
			logging.store(false, std::memory_order_acquire);
			collected_totals.push_back(total);
			total = 0;
		}
		
		uint64_t pkt_id = nethuns_recv(my_socket, &pkthdr, &frame);
		
		if (pkt_id == NETHUNS_ERROR) {
			throw nethuns_exception(my_socket);
		}
		
		if (pkt_id > 0) {
			// Count valid packet
			total++;
			nethuns_rx_release(my_socket, pkt_id);
		}
	}
	
	for (auto t : collected_totals) {
		std::cout << t << std::endl;
	}
}

int main(int argc, char *argv[])
{
	parse_command_line(argc, argv);
	
	signal(SIGINT, terminate);  // register termination signal
	
	// nethuns options
	netopt =
	{
		.numblocks       = 1
	,   .numpackets      = numpackets
	,   .packetsize      = packetsize
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
	
	// Prepare vector for data collection
	collected_totals = std::vector<uint64_t>{};
	collected_totals.reserve((METER_DURATION_SECS / METER_RATE_SECS) + 1);
	
	// case single thread (main) with generic number of sockets
	try {
		execute();
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


void parse_command_line(int argc, char *argv[])
{
	// parse options from command line
	int opt = 0;
	int optidx = 0;
	opterr = 1;     // turn on/off getopt error messages
	if (argc > 1 && argc < 10) {
		while ((opt = getopt_long(argc, argv, "hi:n:s:", NULL, &optidx)) != -1) {
			switch (opt) {
			case 'h':
				std::cout << help_brief << std::endl;
				std::exit(0);
				return;
			case 'i':
				if (optarg)
					interface = optarg;
				break;
			case 'n':
				if (optarg)
					numpackets = atoi(optarg);
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

	std::cout << "\nTest " << argv[0] << " started with parameters \n"
			  << "* interface: " << interface << " \n"
			  << "* numpackets: " << numpackets << " \n"
			  << "* packetsize: " << packetsize << " \n"
			  << std::endl;
}
