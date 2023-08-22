extern crate rayon;
use rayon::prelude::*;

use crate::{convert_pointcloud, StationPoint};
use anyhow::{Context, Result};
use std::{
    fs::File,
    io::{BufWriter, Write as IoWrite},
    path::Path,
    sync::Mutex,
};

pub fn convert_file(input_path: String, output_path: String) -> Result<()> {
    let e57_reader = e57::E57Reader::from_file(&input_path).context("Failed to open e57 file")?;

    if e57_reader.format_name() != "ASTM E57 3D Imaging Data File" {
        return Err(anyhow::anyhow!("Invalid file format"));
    }

    let pointclouds = e57_reader.pointclouds();
    let stations: Mutex<Vec<StationPoint>> = Mutex::new(Vec::new());

    pointclouds
        .par_iter()
        .enumerate()
        .for_each(|(index, pointcloud)| -> () {
            println!("Saving pointcloud {}...", index);
            let mut converter_result =
                match convert_pointcloud(index, pointcloud, &input_path, &output_path) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Error encountered: {}", e);
                        return;
                    }
                };

            stations.lock().unwrap().append(&mut converter_result);
        });

    let stations_file = File::create(Path::new(&output_path).join("stations.json"))?;
    let mut writer = BufWriter::new(stations_file);
    serde_json::to_writer(&mut writer, &stations)?;
    writer.flush()?;

    Ok(())
}