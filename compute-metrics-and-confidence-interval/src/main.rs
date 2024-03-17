use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};

use cli_table::format::{Border, Separator};
use cli_table::Table;
use regex::Regex;
use stats_ci::{quantile, Confidence};


fn main() {
    let mut files: Vec<_> = fs::read_dir("files/")
        .expect("Failed to read directory")
        .map(|file| file.unwrap().path())
        .collect();
    files.sort();
    
    let mut table: Vec<Vec<String>> = Vec::new();
    
    for file in files {
        let data = parse_files(file.as_os_str()).expect("Failed to parse file");
        
        let median = statistical::median(&data);
        let ci = quantile::ci(Confidence::new(0.95), &data, 0.5)
            .expect("Failed to compute confidence interval");
        
        table.push(vec![
            get_experiment_name(file.file_name().unwrap()),
            format!("{:.5}", median),
            format!("({:.5}, {:.5})", ci.low_f(), ci.high_f()),
        ]);
    }
    table.sort();
    
    // Print stats
    print_table(table.clone());
    
    // Compute ratio
    compute_ratio(table);
}


fn parse_files(path: &OsStr) -> Result<Vec<f64>, io::Error> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let vec = reader
        .lines()
        .filter_map(|line| {
            line.expect("Failed to read line").parse::<f64>().ok()
        })
        // Sample taken every 10 seconds, so let's convert it to pkt/sec (pps)
        .map(|x| x / 10.0)
        // Convert to Mpps
        .map(|x| x / 1_000_000.0)
        .collect();
    Ok(vec)
}


fn get_experiment_name(filename: &OsStr) -> String {
    let filename = filename.to_str().unwrap();
    
    let device = if filename.contains("ens") {
        "physical 100G"
    } else if filename.contains("enp") {
        "physical 10G"
    } else if filename.contains("vale") {
        "VALE"
    } else {
        panic!("Unknown device in filename {filename}");
    };
    
    let experiment = if filename.contains("send") {
        "Trasmission"
    } else if filename.contains("recv") {
        "Reception"
    } else {
        panic!("Unknown experiment in filename {filename}");
    };
    
    let language = if filename.contains("cpp") {
        "C++"
    } else if filename.contains("rust") {
        "Rust"
    } else {
        panic!("Unknown language in filename {filename}");
    };
    
    format!("{experiment} ({device}) - {language}")
}


fn print_table(table: Vec<Vec<String>>) {
    println!(
        "{}",
        table
            .table()
            .border(Border::builder().build())
            .separator(Separator::builder().build())
            .display()
            .unwrap()
    );
}


fn compute_ratio(table: Vec<Vec<String>>) {
    let mut cpp_experiment: Option<(String, f64)> = None;
    let re = Regex::new(r"^\w* \(.*\)").unwrap();
    let mut ratio_table: Vec<Vec<String>> = Vec::new();
    
    for row in table.iter() {
        match cpp_experiment {
            None => {
                let name = re.find(row[0].as_str()).unwrap();
                cpp_experiment =
                    Some((name.as_str().to_string(), row[1].parse().unwrap()));
            }
            Some(cpp_data) => {
                let rust_perf = row[1].parse::<f64>().unwrap();
                let cpp_perf = cpp_data.1;
                ratio_table.push(vec![cpp_data.0, format!("{:.4}", rust_perf / cpp_perf)]);
                cpp_experiment = None;
            }
        }
    }
    
    println!("\n\nRATIO");
    print_table(ratio_table);
}
