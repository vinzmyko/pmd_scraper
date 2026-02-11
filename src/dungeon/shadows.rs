use std::{fs, io, path::Path};

use image::{Rgba, RgbaImage};

use super::parse_rgbx_palette;
use crate::containers::binpack::BinPack;

const TILE_BYTES: usize = 32; // 4bpp, 8x8 tile

struct ShadowSprite {
    name: &'static str,
    tile_indices: &'static [usize],
    cols: usize,
    rows: usize,
}

const SHADOW_SPRITES: [ShadowSprite; 6] = [
    ShadowSprite {
        name: "enemy_small",
        tile_indices: &[0],
        cols: 1,
        rows: 1,
    },
    ShadowSprite {
        name: "enemy_medium",
        tile_indices: &[1, 2],
        cols: 2,
        rows: 1,
    },
    ShadowSprite {
        name: "enemy_large",
        tile_indices: &[3, 4, 5, 6],
        cols: 4,
        rows: 1,
    },
    ShadowSprite {
        name: "ally_small",
        tile_indices: &[7],
        cols: 1,
        rows: 1,
    },
    ShadowSprite {
        name: "ally_medium",
        tile_indices: &[8, 9],
        cols: 2,
        rows: 1,
    },
    ShadowSprite {
        name: "ally_large",
        tile_indices: &[10, 11, 12, 13, 14, 15, 16, 17],
        cols: 4,
        rows: 2,
    },
];

pub fn extract_shadows(binpack: &BinPack, output_dir: &Path) -> io::Result<()> {
    let raw_995 = binpack
        .get(995)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Shadow texture (995) not found"))?;
    let raw_997 = binpack
        .get(997)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Shadow palette (997) not found"))?;

    let palette = parse_rgbx_palette(raw_997);

    let tile_count = u32::from_le_bytes(raw_995[0..4].try_into().unwrap()) as usize;
    let tile_data = &raw_995[4..];

    if tile_count != 50 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Expected 50 shadow tiles, got {}", tile_count),
        ));
    }

    fs::create_dir_all(output_dir)?;

    for sprite in SHADOW_SPRITES.iter() {
        let img = assemble_sprite(sprite, tile_data, &palette);
        img.save(output_dir.join(format!("{}.png", sprite.name)))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        println!(
            "  -> {}.png ({}x{})",
            sprite.name,
            img.width(),
            img.height()
        );
    }

    Ok(())
}

fn assemble_sprite(sprite: &ShadowSprite, tile_data: &[u8], palette: &[Rgba<u8>]) -> RgbaImage {
    let width = (sprite.cols * 8) as u32;
    let height = (sprite.rows * 8) as u32;
    let mut img = RgbaImage::new(width, height);

    for (i, &tile_idx) in sprite.tile_indices.iter().enumerate() {
        let tx = ((i % sprite.cols) * 8) as u32;
        let ty = ((i / sprite.cols) * 8) as u32;
        decode_4bpp_tile(tile_idx, tile_data, palette, &mut img, tx, ty);
    }

    img
}

fn decode_4bpp_tile(
    tile_idx: usize,
    tile_data: &[u8],
    palette: &[Rgba<u8>],
    img: &mut RgbaImage,
    ox: u32,
    oy: u32,
) {
    let offset = tile_idx * TILE_BYTES;
    let tile = &tile_data[offset..offset + TILE_BYTES];

    for (byte_idx, &byte) in tile.iter().enumerate() {
        let px = ((byte_idx % 4) * 2) as u32;
        let py = (byte_idx / 4) as u32;

        let lo = (byte & 0x0F) as usize;
        let hi = ((byte >> 4) & 0x0F) as usize;

        if lo < palette.len() {
            img.put_pixel(ox + px, oy + py, palette[lo]);
        }
        if hi < palette.len() {
            img.put_pixel(ox + px + 1, oy + py, palette[hi]);
        }
    }
}
