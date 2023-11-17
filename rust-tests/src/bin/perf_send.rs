use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::{env, thread};

use nethuns::sockets::{BindableNethunsSocket, Local, NethunsSocket};
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
}


const HELP_BRIEF: &str = "\
Usage:  send [ options ]
Use --help (or -h) to see full option list and a complete description

Required options:
            [ -i <ifname> ]     set network interface
Other options:
            [ -b <batch_sz> ]   set batch size
            [ -z ]              enable send zero-copy
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
            std::process::exit(0);
        }
    };
    
    
    // Nethuns options
    let opt = NethunsSocketOptions {
        numblocks: 1,
        numpackets: 1024,
        packetsize: 0,
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
    let socket: NethunsSocket<Local> =
        prepare_tx_socket(&args, opt, &PAYLOAD).unwrap();
    let mut pktid: usize = 0; // pos of next slot/packet to send in tx ring
    
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
    
    loop {
        // Check condition for program termination
        if term.load(Ordering::Relaxed) {
            break;
        }
        
        // Transmit packets from each socket
        if args.zerocopy {
            transmit_zc(&args, &socket, &mut pktid, PAYLOAD.len(), &total)
                .unwrap();
        } else {
            transmit_c(&args, &socket, &PAYLOAD, &total).unwrap();
        }
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
) -> Result<NethunsSocket<Local>, anyhow::Error> {
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

/// Transmit packets in the tx ring (use optimized send, zero copy).
fn transmit_zc(
    args: &Args,
    socket: &NethunsSocket<Local>,
    pktid: &mut usize,
    pkt_size: usize,
    total: &Arc<AtomicU64>,
) -> Result<(), anyhow::Error> {
    // Prepare batch
    for _ in 0..args.batch_size {
        if socket.send_slot(*pktid, pkt_size).is_err() {
            break;
        }
        *pktid += 1;
        total.fetch_add(1, Ordering::SeqCst);
    }
    // Send batch
    socket.flush()?;
    Ok(())
}

/// Transmit packets in the tx ring (use classic send, copy)
///
/// # Arguments
/// - `args`: Parsed command-line arguments.
/// - `socket`: Socket descriptor.
/// - `payload`: Payload for packets.
/// - `totals`: Vector for storing the number of packets sent from each socket.
/// - `socket_idx`: Socket index.
fn transmit_c(
    args: &Args,
    socket: &NethunsSocket<Local>,
    payload: &[u8],
    total: &Arc<AtomicU64>,
) -> Result<(), anyhow::Error> {
    // Prepare batch
    for _ in 0..args.batch_size {
        if socket.send(payload).is_err() {
            break;
        }
        total.fetch_add(1, Ordering::SeqCst);
    }
    // Send batch
    socket.flush()?;
    Ok(())
}

/// Collect stats about the received packets every `METER_RATE_SECS` seconds
fn meter(total: Arc<AtomicU64>, term: Arc<AtomicBool>) {
    let mut now = SystemTime::now();
    
    loop {
        if term.load(Ordering::Relaxed) {
            break;
        }
        
        // Sleep for 1 second
        let next_sys_time = now
            .checked_add(Duration::from_secs(METER_RATE_SECS))
            .expect("SystemTime::checked_add() failed");
        if let Ok(delay) = next_sys_time.duration_since(now) {
            thread::sleep(delay);
        }
        now = next_sys_time;
        
        let total = total.swap(0, Ordering::SeqCst);
        
        // Print number of sent packets
        println!("{}", total);
    }
}
