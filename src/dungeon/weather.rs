//! Weather visual assets from dungeon.bin: the 3D overlay textures (fog,
//! sandstorm, tileset mist, poison-mist) and the per-weather colour table.

use std::{fs, io, path::Path};

use image::{Rgba, RgbaImage};

use super::colvec::{Colvec, COLVEC_COLORS, WEATHER_COUNT};
use crate::{
    containers::{binpack::BinPack, sir0::Sir0},
    graphics::wte::Wte,
};

/// dungeon.bin index of the weather colour table.
const COLVEC_INDEX: usize = 1034;

/// (dungeon.bin index, output filename) for each 3D overlay texture.
const WTE_TEXTURES: &[(usize, &str)] = &[
    (1001, "fog_1001.png"),
    (1005, "sandstorm_1005.png"),
    (1031, "mist_1031.png"),
    (1003, "poison_mist_1003.png"),
];

pub fn extract_weather_assets(binpack: &BinPack, output_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(output_dir)?;
    extract_textures(binpack, output_dir)?;
    extract_colvec(binpack, output_dir)?;
    Ok(())
}

fn extract_textures(binpack: &BinPack, output_dir: &Path) -> io::Result<()> {
    for &(index, filename) in WTE_TEXTURES {
        let raw = binpack.get(index).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("WTE texture {} not found in dungeon.bin", index),
            )
        })?;

        let sir0 = Sir0::from_bytes(raw)?;
        let wte = Wte::from_sir0_content(&sir0.content, sir0.data_pointer)?;
        let img = wte.to_rgba()?;

        let path = output_dir.join(filename);
        img.save(&path)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        println!("  -> {} ({}x{})", filename, img.width(), img.height());
    }
    Ok(())
}

fn extract_colvec(binpack: &BinPack, output_dir: &Path) -> io::Result<()> {
    let raw = binpack.get(COLVEC_INDEX).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("colvec ({}) not found in dungeon.bin", COLVEC_INDEX),
        )
    })?;

    let sir0 = Sir0::from_bytes(raw)?;
    let colvec = Colvec::from_sir0_content(&sir0.content)?;

    // LUT texture: 256 wide (input value) x 8 tall (weather_id).
    // Pixel (v, w) = colormap[w][v]; the client samples each output channel
    // independently: out_R = lut(in_R, w).r, out_G = lut(in_G, w).g, etc.
    let mut lut = RgbaImage::new(COLVEC_COLORS as u32, WEATHER_COUNT as u32);
    for (w, map) in colvec.colormaps.iter().enumerate() {
        for (v, &(r, g, b)) in map.iter().enumerate() {
            lut.put_pixel(v as u32, w as u32, Rgba([r, g, b, 255]));
        }
    }

    let path = output_dir.join("colvec.png");
    lut.save(&path)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    println!("  -> colvec.png ({}x{})", lut.width(), lut.height());
    Ok(())
}
