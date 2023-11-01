use std::fs::File;
use std::io::{self, BufRead, BufReader};

use plotly::color::NamedColor;
use plotly::common::{DashType, Title};
use plotly::layout::{Axis, Shape, ShapeLine, ShapeType};
use plotly::{BoxPlot, ImageFormat, Layout, Plot};


const RUST_DATA_FILENAME: &str = "files/rust_data.txt";
const CPP_DATA_FILENAME: &str = "files/cpp_data.txt";
const OUTPUT_FILENAME: &str = "files/out.png";

const REFERENCE_VALUE_Y: f64 = 12.0;


fn main() {
    generate_box_plot(
        parse_collected_data(RUST_DATA_FILENAME)
            .expect("Failed to parse Rust data"),
        parse_collected_data(CPP_DATA_FILENAME)
            .expect("Failed to parse C++ data"),
        OUTPUT_FILENAME,
    );
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
    // plot.show_image(ImageFormat::SVG, 1080, 720);
    plot.write_image(output_filename, ImageFormat::PNG, 1000, 500, 4.0);
}


fn generate_layout() -> Layout {
    let mut layout = Layout::new()
        .height(500)
        .width(1000)
        .y_axis(
            Axis::new()
                .title(Title::new("Throughput (Mpps)")),
        )
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
