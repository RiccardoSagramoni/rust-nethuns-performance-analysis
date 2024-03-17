use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::{env, thread};

use nethuns::sockets::{BindableNethunsSocket, NethunsSocket};
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const METER_DURATION_SECS: u64 = 10 * 60 + 1;
const METER_RATE_SECS: u64 = 10;

#[derive(Debug)]
struct Args {
    interface: String,
    batch_size: u32,
    zerocopy: bool,
    
    numpackets: u32,
    packetsize: u32,
}


const HELP_BRIEF: &str = "\
Usage:  send [ options ]
Use --help (or -h) to see full option list and a complete description

Required options:
            [ -i <ifname> ]                     set network interface
Other options:
            [ -b <batch_sz> ]                   set batch size
            [ -z ]                              enable send zero-copy
            [ --numpackets <num_packets> ]      set number of packets (default 1024)
            [ --packetsize <packet_size> ]      set packet size (default 0)
";

const PAYLOAD: [u8; 34] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf0, 0xbf, /* L`..UF.. */
    0x97, 0xe2, 0xff, 0xae, 0x08, 0x00, 0x45, 0x00, /* ......E. */
    0x00, 0x54, 0xb3, 0xf9, 0x40, 0x00, 0x40, 0x11, /* .T..@.@. */
    0xf5, 0x32, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, /* .2...... */
    0x07, 0x08,
];


fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();
    
    // Parse options from command line
    let args = match parse_args() {
        Ok(args) => {
            println!(
                "Test {} started with parameters: \n{:#?}",
                env::args().next().unwrap(),
                args
            );
            args
        }
        Err(e) => {
            eprintln!("Error in parsing command line options: {e}.");
            eprint!("{}", HELP_BRIEF);
            return;
        }
    };
    
    
    // Nethuns options
    let opt = NethunsSocketOptions {
        numblocks: 1,
        numpackets: args.numpackets,
        packetsize: args.packetsize,
        timeout_ms: 0,
        dir: NethunsCaptureDir::InOut,
        capture: NethunsCaptureMode::ZeroCopy,
        mode: NethunsSocketMode::RxTx,
        promisc: false,
        rxhash: false,
        tx_qdisc_bypass: true,
        ..Default::default()
    };
    
    
    // Setup and fill transmission rings for each socket
    let socket = prepare_tx_socket(&args, opt, &PAYLOAD).unwrap();
    
    // Define atomic variable for program termination
    let term = Arc::new(AtomicBool::new(false));
    
    // Set handler for Ctrl-C
    set_sigint_handler(term.clone());
    
    // Set timer for stopping data collection after 10 minutes
    let _ = {
        let term = term.clone();
        let stop_time = SystemTime::now()
            .checked_add(Duration::from_secs(METER_DURATION_SECS))
            .unwrap();
        thread::spawn(move || {
            if let Ok(delay) = stop_time.duration_since(SystemTime::now()) {
                thread::sleep(delay);
            }
            term.store(true, Ordering::Relaxed);
        })
    };
    
    let total = Arc::new(AtomicU64::new(0));
    let _ = {
        let total = total.clone();
        let term = term.clone();
        thread::spawn(move || {
            meter(total, term);
        })
    };
    
    let mut local_total: u64 = 0;
    
    while term.load(Ordering::Relaxed) {
        // Prepare batch
        for _ in 0..args.batch_size {
            if socket.send(&PAYLOAD).is_err() {
                break;
            }
            local_total += 1;
            
            if local_total & 0x3FF == 0 {
                // update every 1024 packets
                total.store(local_total, Ordering::Release);
            }
        }
        
        // Send batch
        socket.flush().expect("flush failed");
    }
}

/// Parses the command-line arguments and build an instance of the `Args`
/// struct.
///
/// It uses the `pico_args` crate to handle argument parsing.
///
/// # Returns
///
/// - `Ok(Args)`: If the command-line arguments are successfully parsed, a
///   Result with an Args instance containing the parsed options is returned.
/// - `Err(anyhow::Error)`: If an error occurs during argument parsing or
///   any related operations, a Result with a boxed error is returned.
fn parse_args() -> Result<Args, anyhow::Error> {
    let mut pargs = pico_args::Arguments::from_env();
    
    // Help has a higher priority and should be handled separately.
    if pargs.contains(["-h", "--help"]) {
        print!("{}", HELP_BRIEF);
        std::process::exit(0);
    }
    
    let args = Args {
        interface: pargs.value_from_str(["-i", "--interface"])?,
        batch_size: pargs.value_from_str(["-b", "--batch_size"]).unwrap_or(1),
        zerocopy: pargs.contains(["-z", "--zerocopy"]),
        numpackets: pargs.value_from_str("--numpackets").unwrap_or(1024),
        packetsize: pargs.value_from_str("--packetsize").unwrap_or(0),
    };
    
    // It's up to the caller what to do with the remaining arguments.
    let remaining = pargs.finish();
    if !remaining.is_empty() {
        eprintln!("Warning: unused arguments left: {:?}.", remaining);
    }
    
    Ok(args)
}

/// Set an handler for the SIGINT signal (Ctrl-C),
/// which will notify the other threads
/// to gracefully stop their execution.
fn set_sigint_handler(term: Arc<AtomicBool>) {
    ctrlc::set_handler(move || {
        println!("Ctrl-C detected. Shutting down...");
        term.store(true, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");
}

/// Setup and fill transmission ring.
fn prepare_tx_socket(
    args: &Args,
    opt: NethunsSocketOptions,
    payload: &[u8],
) -> Result<NethunsSocket, anyhow::Error> {
    // Open socket
    let mut socket = BindableNethunsSocket::open(opt)?
        .bind(&args.interface, NethunsQueue::Any)
        .map_err(|(e, _)| e)?;
    
    // fill the slots in the tx ring (optimized send only)
    if args.zerocopy {
        let size = socket.txring_get_size().expect("socket not in tx mode");
        
        for j in 0..size {
            // tell me where to copy the j-th packet to be transmitted
            let mut pkt = socket
                .get_packet_buffer_ref(j as _)
                .expect("socket not in tx mode");
            
            // copy the packet
            pkt.write_all(payload)?;
        }
    }
    
    Ok(socket)
}


/// Collect stats about the received packets every `METER_RATE_SECS` seconds
fn meter(total: Arc<AtomicU64>, term: Arc<AtomicBool>) {
    let mut now = SystemTime::now();
    
    while term.load(Ordering::Relaxed) {
        // Sleep for `METER_RATE_SECS` second
        now = sleep(now);
        
        // Print number of sent packets
        println!("{}", total.load(Ordering::Acquire));
    }
}

/// Sleep for `METER_RATE_SECS` second
fn sleep(now: SystemTime) -> SystemTime {
    let next_sys_time = now
        .checked_add(Duration::from_secs(METER_RATE_SECS))
        .expect("SystemTime::checked_add() failed");
    
    if let Ok(delay) = next_sys_time.duration_since(now) {
        thread::sleep(delay);
    }
    
    next_sys_time
}
