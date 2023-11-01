use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::{env, mem};

use plotly::color::NamedColor;
use plotly::common::{DashType, Title};
use plotly::layout::{Axis, Shape, ShapeLine, ShapeType};
use plotly::{BoxPlot, ImageFormat, Layout, Plot};


const REFERENCE_VALUE_Y: f64 = 12.0;

#[derive(Debug)]
struct Config {
    rust_data_filename: String,
    cpp_data_filename: String,
    output_filename: String,
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
    println!("Rust stats:\n{:#?}\n", compute_stats(&rust_data));
    println!("C++ stats:\n{:#?}\n", compute_stats(&cpp_data));
    
    // Generate plot
    generate_box_plot(rust_data, cpp_data, &config.output_filename);
}


fn get_config() -> Config {
    let mut args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        panic!("Usage: {} <rust_data_filename> <cpp_data_filename> <output_filename>", args[0]);
    }
    
    Config {
        rust_data_filename: mem::take(&mut args[1]),
        cpp_data_filename: mem::take(&mut args[2]),
        output_filename: mem::take(&mut args[3]),
    }
}


fn parse_collected_data(filename: &str) -> Result<Vec<f64>, io::Error> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let vec = reader
        .lines()
        .filter_map(|line| {
            line.expect("Failed to read line").parse::<u64>().ok()
        })
        .map(|x| (x as f64) / 1_000_000.0)
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
) {
    let mut plot = Plot::new();
    
    plot.add_trace(BoxPlot::new(rust_data).name("Rust Nethuns"));
    plot.add_trace(BoxPlot::new(cpp_data).name("C++ Nethuns"));
    
    plot.set_layout(generate_layout());
    
    plot.show();
    plot.write_image(output_filename, ImageFormat::PNG, 1000, 500, 4.0);
}


fn generate_layout() -> Layout {
    let mut layout = Layout::new()
        .height(500)
        .width(1000)
        .y_axis(Axis::new().title(Title::new("Throughput (Mpps)")))
        .show_legend(false);
    
    // Add reference line
    layout.add_shape(
        Shape::new()
            .shape_type(ShapeType::Line)
            .x0(-1)
            .x1(2)
            .y0(REFERENCE_VALUE_Y)
            .y1(REFERENCE_VALUE_Y)
            .line(
                ShapeLine::new()
                    .color(NamedColor::DarkBlue)
                    .width(2.)
                    .dash(DashType::DashDot),
            ),
    );
    
    layout
}
