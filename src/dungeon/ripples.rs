use std::{fs, io, path::Path};

use image::{Rgba, RgbaImage};

use super::parse_rgbx_palette;
use crate::containers::{binpack::BinPack, sir0::Sir0};

const TILE_BYTES_8BPP: usize = 64; // 8x8 pixels, 1 byte per pixel
const ENEMY_FRAME_SIZE: usize = 0x100; // 256 bytes = 4 tiles
const ALLY_FRAME_SIZE: usize = 0x200; // 512 bytes = 8 tiles
const NUM_FRAMES: usize = 3;

const ENEMY_COLS: usize = 4;
const ENEMY_ROWS: usize = 1;
const ALLY_COLS: usize = 4;
const ALLY_ROWS: usize = 2;

pub fn extract_ripples(binpack: &BinPack, output_dir: &Path) -> io::Result<()> {
    let raw_996 = binpack
        .get(996)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Ripple data (996) not found"))?;
    let raw_997 = binpack
        .get(997)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Ripple palette (997) not found"))?;

    let sir0 = Sir0::from_bytes(raw_996)?;
    let content = &sir0.content;

    let expected_size = NUM_FRAMES * (ENEMY_FRAME_SIZE + ALLY_FRAME_SIZE);
    if content.len() < expected_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Ripple content too short: {} < {}",
                content.len(),
                expected_size
            ),
        ));
    }

    let palette = parse_rgbx_palette(raw_997);

    fs::create_dir_all(output_dir)?;

    // Enemy sheet: 3 frames of 32x8 side by side = 96x8
    let enemy_width = (ENEMY_COLS * 8 * NUM_FRAMES) as u32;
    let enemy_height = (ENEMY_ROWS * 8) as u32;
    let mut enemy_sheet = RgbaImage::new(enemy_width, enemy_height);

    // Ally sheet: 3 frames of 32x16 side by side = 96x16
    let ally_width = (ALLY_COLS * 8 * NUM_FRAMES) as u32;
    let ally_height = (ALLY_ROWS * 8) as u32;
    let mut ally_sheet = RgbaImage::new(ally_width, ally_height);

    for frame in 0..NUM_FRAMES {
        let frame_offset = frame * (ENEMY_FRAME_SIZE + ALLY_FRAME_SIZE);

        let enemy_data = &content[frame_offset..frame_offset + ENEMY_FRAME_SIZE];
        let ally_data = &content
            [frame_offset + ENEMY_FRAME_SIZE..frame_offset + ENEMY_FRAME_SIZE + ALLY_FRAME_SIZE];

        let enemy_x = (frame * ENEMY_COLS * 8) as u32;
        render_8bpp_tiles(
            enemy_data,
            ENEMY_COLS,
            ENEMY_ROWS,
            &palette,
            &mut enemy_sheet,
            enemy_x,
            0,
        );

        let ally_x = (frame * ALLY_COLS * 8) as u32;
        render_8bpp_tiles(
            ally_data,
            ALLY_COLS,
            ALLY_ROWS,
            &palette,
            &mut ally_sheet,
            ally_x,
            0,
        );
    }

    enemy_sheet
        .save(output_dir.join("enemy_ripple.png"))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    println!("  -> enemy_ripple.png ({}x{})", enemy_width, enemy_height);

    ally_sheet
        .save(output_dir.join("ally_ripple.png"))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    println!("  -> ally_ripple.png ({}x{})", ally_width, ally_height);

    Ok(())
}

fn render_8bpp_tiles(
    data: &[u8],
    cols: usize,
    rows: usize,
    palette: &[Rgba<u8>],
    img: &mut RgbaImage,
    ox: u32,
    oy: u32,
) {
    for tile_idx in 0..(cols * rows) {
        let tx = ox + ((tile_idx % cols) * 8) as u32;
        let ty = oy + ((tile_idx / cols) * 8) as u32;
        let tile_start = tile_idx * TILE_BYTES_8BPP;
        let tile_data = &data[tile_start..tile_start + TILE_BYTES_8BPP];

        for (pixel_idx, &byte) in tile_data.iter().enumerate() {
            let px = (pixel_idx % 8) as u32;
            let py = (pixel_idx / 8) as u32;

            let idx = if byte == 0 { 0 } else { (byte & 0x0F) as usize };

            if idx < palette.len() {
                img.put_pixel(tx + px, ty + py, palette[idx]);
            }
        }
    }
}
