mod extended_point;
mod utils;
extern crate rayon;
use anyhow::{Context, Result};
use clap::Parser;
use e57::E57Reader;
use extended_point::ExtendedPoint;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use las::Write;
use rayon::prelude::*;
use serde::Serialize;
use std::{
    fs::File,
    io::{BufWriter, Write as IoWrite},
    path::Path,
    sync::Mutex,
};
use uuid::Uuid;

use crate::utils::*;

#[derive(Serialize)]
struct StationPoint {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    path: String,

    #[arg(short, long, default_value_t = String::from("./"))]
    output: String,

    #[arg(short = 'P', long, default_value_t = false)]
    progress: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let input_path = args.path;
    let output_path = Path::new(&args.output);
    let has_progress = args.progress;

    let e57_reader = E57Reader::from_file(&input_path).context("Failed to open e57 file")?;

    if e57_reader.format_name() != "ASTM E57 3D Imaging Data File" {
        println!("This file is currently not supported.");
        return Ok(());
    }

    if true {
        // Write the xml header in header.xml
        let mut header_file = BufWriter::new(File::create("header.xml").unwrap());
        header_file.write_all(e57_reader.xml().as_bytes()).unwrap();
        header_file.flush().unwrap();
    }

    let pointclouds = e57_reader.pointclouds();

    let mut progress_bar = ProgressBar::hidden();

    let stations: Mutex<Vec<StationPoint>> = Mutex::new(Vec::new());

    if has_progress {
        let total_records: u64 = pointclouds.iter().map(|pc| pc.records).sum();
        progress_bar = ProgressBar::new(total_records);
        progress_bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {msg} ({eta})",
            )
            .unwrap()
            .with_key(
                "eta",
                |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                    write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
                },
            )
            .progress_chars("=>"),
        );
    }

    pointclouds
        .par_iter()
        .enumerate()
        .for_each(|(index, pointcloud)| -> () {
            let las_path = match construct_las_path(&output_path, index)
                .context("Couldn't create las path.")
            {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Error encountered: {}", e);
                    return ();
                }
            };

            let mut builder = las::Builder::from((1, 4));
            builder.point_format.has_color = true;
            builder.generating_software = String::from("e57_to_las");
            builder.guid = match Uuid::parse_str(&pointcloud.guid.clone().replace("_", "-")) {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("Invalid guid: {}", e);
                    return ();
                }
            };

            let header = match builder.into_header() {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("Error encountered: {}", e);
                    return ();
                }
            };

            let mut writer = match las::Writer::from_path(&las_path, header) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("Error encountered: {}", e);
                    return ();
                }
            };

            let mut e57_reader =
                match E57Reader::from_file(&input_path).context("Failed to open e57 file") {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Error encountered: {}", e);
                        return ();
                    }
                };

            let mut pointcloud_reader = match e57_reader
                .pointcloud_simple(&pointcloud)
                .context("Unable to get point cloud iterator")
            {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("Error encountered: {}", e);
                    return ();
                }
            };
            pointcloud_reader.skip_invalid(true);

            println!("Saving pointcloud {} ...", index);
            let mut count = 0.0;
            let mut sum_coordinate = (0.0, 0.0, 0.0);

            pointcloud_reader.for_each(|p| {
                let point = match p {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Error encountered: {}", e);
                        return ();
                    }
                };
                count += 1.0;
                sum_coordinate = (
                    sum_coordinate.0 + point.cartesian.x,
                    sum_coordinate.1 + point.cartesian.y,
                    sum_coordinate.2 + point.cartesian.z,
                );

                let las_rgb = ExtendedPoint::from(point.clone()).rgb_color;
                let las_intensity = get_intensity(point.intensity, point.intensity_invalid);

                let las_point = las::Point {
                    x: point.cartesian.x,
                    y: point.cartesian.y,
                    z: point.cartesian.z,
                    intensity: las_intensity,
                    color: Some(las_rgb),
                    ..Default::default()
                };

                match writer.write(las_point) {
                    Ok(_) => (),
                    Err(_e) => return,
                };

                progress_bar.inc(1);
            });

            match writer.close() {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("Error encountered: {}", e);
                    return ();
                }
            };

            stations.lock().unwrap().push(StationPoint {
                x: sum_coordinate.0 / count,
                y: sum_coordinate.1 / count,
                z: sum_coordinate.2 / count,
            });
        });

    let stations_file = File::create(output_path.join("stations.json"))?;
    let mut writer = BufWriter::new(stations_file);
    serde_json::to_writer(&mut writer, &stations)?;
    writer.flush()?;

    progress_bar.finish_with_message("Finished convertion from e57 to las !");
    Ok(())
}
