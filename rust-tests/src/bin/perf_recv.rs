use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::{env, thread};

use nethuns::sockets::errors::NethunsRecvError;
use nethuns::sockets::BindableNethunsSocket;
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const METER_DURATION_SECS: u64 = 10 * 60 + 1;
const METER_RATE_SECS: u64 = 10;

const HELP_BRIEF: &str = "\
Usage:  recv [ options ]
Use --help (or -h) to see full option list and a complete description

Required options:
            [ -i <ifname> ]                     set network interface
Other options:
            [ --numpackets <num_packets> ]      set number of packets (default 1024)
            [ --packetsize <packet_size> ]      set packet size (default 0)
";


#[derive(Debug)]
struct Args {
    interface: String,
    numpackets: u32,
    packetsize: u32,
}


fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();
    
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
    
    let nethuns_opt = NethunsSocketOptions {
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
    
    // Open sockets
    let socket = BindableNethunsSocket::open(nethuns_opt)
        .unwrap()
        .bind(&args.interface, NethunsQueue::Any)
        .unwrap();
    
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
    
    // Start thread for data collection
    let total = Arc::new(AtomicU64::new(0));
    
    let _ = {
        let term = term.clone();
        let total = total.clone();
        thread::spawn(move || {
            meter(total, term);
        })
    };
    
    let mut local_total: u64 = 0;
    
    // Start receiving
    while !term.load(Ordering::Relaxed) {
        match socket.recv() {
            Ok(_) => {
                local_total += 1;
                
                if local_total & 0x3FF == 0 {
                    // update counter every 1024 packets
                    total.store(local_total, Ordering::Release);
                }
            }
            
            Err(NethunsRecvError::InUse)
            | Err(NethunsRecvError::NoPacketsAvailable)
            | Err(NethunsRecvError::PacketFiltered) => (),
            
            Err(e) => panic!("Error: {e}"),
        }
    }
}


fn parse_args() -> anyhow::Result<Args> {
    let mut pargs = pico_args::Arguments::from_env();
    
    // Help has a higher priority and should be handled separately.
    if pargs.contains(["-h", "--help"]) {
        print!("{}", HELP_BRIEF);
        std::process::exit(0);
    }
    
    let args = Args {
        interface: pargs.value_from_str("-i")?,
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


/// Collect stats about the received packets every `METER_RATE_SECS` seconds
fn meter(total: Arc<AtomicU64>, term: Arc<AtomicBool>) {
    let mut now = SystemTime::now();
    let mut old_total: u64 = 0;
    
    while !term.load(Ordering::Relaxed) {
        // Sleep for `METER_RATE_SECS` second
        now = sleep(now);
        
        // Print number of sent packets
        let new_total = total.load(Ordering::Acquire);
        println!("{}", new_total - old_total);
        old_total = new_total;
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
