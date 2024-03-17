use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::{env, mem};

use plotly::common::{Font, Title};
use plotly::layout::Axis;
use plotly::{BoxPlot, ImageFormat, Layout, Plot};


#[derive(Debug)]
struct Config {
    rust_data_filename: String,
    cpp_data_filename: String,
    output_filename: String,
    title: String,
}


#[allow(dead_code)]
#[derive(Debug)]
struct Stats {
    mean: f64,
    median: f64,
    population_standard_deviation: f64,
    population_variance: f64,
    standard_deviation: f64,
    variance: f64,
}


fn main() {
    let config = get_config();
    println!(
        "Start generating plots with the following configuration:\n{:#?}\n",
        config
    );
    
    // Parse data
    let rust_data = parse_collected_data(&config.rust_data_filename)
        .expect("Failed to parse Rust data");
    let cpp_data = parse_collected_data(&config.cpp_data_filename)
        .expect("Failed to parse C++ data");
    
    // Print stats
    let rust_stats = compute_stats(&rust_data);
    let cpp_stats = compute_stats(&cpp_data);
    println!("Rust stats:\n{rust_stats:#?}\n");
    println!("C++ stats:\n{cpp_stats:#?}\n");
    println!("Ratio Rust / C++: {}", rust_stats.median / cpp_stats.median);
    
    // Generate plot
    generate_box_plot(
        rust_data,
        cpp_data,
        &config.output_filename,
        &config.title,
    );
}


fn get_config() -> Config {
    let mut args: Vec<String> = env::args().collect();
    if args.len() < 5 {
        panic!("Usage: {} <rust_data_filename> <cpp_data_filename> <output_filename> <title>", args[0]);
    }
    
    Config {
        rust_data_filename: mem::take(&mut args[1]),
        cpp_data_filename: mem::take(&mut args[2]),
        output_filename: mem::take(&mut args[3]),
        title: mem::take(&mut args[4]),
    }
}


fn parse_collected_data(filename: &str) -> Result<Vec<f64>, io::Error> {
    let file = File::open(filename)?;
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


fn compute_stats(data: &[f64]) -> Stats {
    Stats {
        mean: statistical::mean(data),
        median: statistical::median(data),
        population_standard_deviation:
            statistical::population_standard_deviation(data, None),
        population_variance: statistical::population_variance(data, None),
        standard_deviation: statistical::standard_deviation(data, None),
        variance: statistical::variance(data, None),
    }
}


fn generate_box_plot(
    rust_data: Vec<f64>,
    cpp_data: Vec<f64>,
    output_filename: &str,
    title: &str,
) {
    let mut plot = Plot::new();
    
    plot.add_trace(
        BoxPlot::new(rust_data)
            .name("Rust Nethuns")
            .whisker_width(0.2),
    );
    plot.add_trace(BoxPlot::new(cpp_data).name("C Nethuns").whisker_width(0.2));
    
    plot.set_layout(generate_layout(title));
    
    // plot.show();
    plot.write_html(format!("{output_filename}.html"));
    plot.write_image(output_filename, ImageFormat::PNG, 800, 500, 4.0);
}


fn generate_layout(title: &str) -> Layout {
    Layout::new()
        .y_axis(
            Axis::new()
                .title(
                    Title::new("Throughput (Mpps)").font(Font::new().size(16)),
                )
                .color("black")
                // .range(vec![start, end])
                .dtick(0.2),
        )
        .x_axis(Axis::new().color("black").tick_font(Font::new().size(16)))
        .show_legend(false)
        .title(
            Title::new(format!("<b>{title}</b>").as_str())
                .font(Font::new().color("black").size(24)),
        )
}
