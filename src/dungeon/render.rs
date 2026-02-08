//! # Dungeon Tileset Visualisation
//!
//! Converts the parsed dungeon data structures (DMA, DPC, DPCI, DPL, DPLA) into
//! usable assets for external applications.
//!
//! ## Responsibilities
//! - Rasterisation: Decodes 4bpp tile indices and maps them to RGB colours.
//! - Composition: Assembles 3x3 hardware tiles into 24x24 chunks.
//! - Export: Generates indexed PNG spritesheets and JSON metadata.

use std::{fs, io, path::Path};

use image::{Rgba, RgbaImage};
use serde::Serialize;

use super::{dma::DmaType, dpc::DPC_TILES_PER_CHUNK, dpci::DPCI_TILE_DIM, DungeonTileset};

const CHUNK_PX: usize = DPCI_TILE_DIM * 3; // 24
const SHEET_COLS: usize = 16;

#[derive(Serialize)]
pub struct TilesetMetadata {
    pub tileset_id: usize,
    pub chunk_count: usize,
    pub sheet_width: usize,
    pub sheet_height: usize,
    pub chunk_size: usize,
    pub dma_rules: DmaRules,
    pub palettes: Vec<Vec<[u8; 3]>>,
    pub animation: Option<AnimationMetadata>,
}

/// 256 neighbor configs × 3 variations per tile type
#[derive(Serialize)]
pub struct DmaRules {
    pub wall: Vec<[u8; 3]>,
    pub secondary: Vec<[u8; 3]>,
    pub floor: Vec<[u8; 3]>,
}

#[derive(Serialize)]
pub struct AnimationMetadata {
    pub palette_10: Vec<ColourAnimation>,
    pub palette_11: Vec<ColourAnimation>,
}

#[derive(Serialize)]
pub struct ColourAnimation {
    pub colour_index: usize,
    pub duration_frames: u16,
    pub frames: Vec<[u8; 3]>,
}

pub fn render_tileset(tileset: &DungeonTileset, output_dir: &Path) -> Result<(), io::Error> {
    fs::create_dir_all(output_dir)?;

    let sheet = render_chunk_sheet(tileset);
    sheet
        .save(output_dir.join("chunks.png"))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let metadata = build_metadata(tileset, sheet.width() as usize, sheet.height() as usize);
    let json = serde_json::to_string_pretty(&metadata)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(output_dir.join("tileset.json"), json)?;

    Ok(())
}

fn render_chunk_sheet(tileset: &DungeonTileset) -> RgbaImage {
    let count = tileset.dpc.chunks.len();
    let rows = (count + SHEET_COLS - 1) / SHEET_COLS;
    let mut img = RgbaImage::new((SHEET_COLS * CHUNK_PX) as u32, (rows * CHUNK_PX) as u32);

    for (idx, chunk) in tileset.dpc.chunks.iter().enumerate() {
        let bx = (idx % SHEET_COLS) * CHUNK_PX;
        let by = (idx / SHEET_COLS) * CHUNK_PX;
        render_chunk(&mut img, tileset, chunk, bx, by);
    }
    img
}

fn render_chunk(
    img: &mut RgbaImage,
    tileset: &DungeonTileset,
    chunk: &[super::dpc::TileMapping; DPC_TILES_PER_CHUNK],
    base_x: usize,
    base_y: usize,
) {
    for (i, mapping) in chunk.iter().enumerate() {
        let tx = base_x + (i % 3) * DPCI_TILE_DIM;
        let ty = base_y + (i / 3) * DPCI_TILE_DIM;

        let ti = mapping.tile_index as usize;
        if ti >= tileset.dpci.tiles.len() {
            continue;
        }

        let pixels = tileset.dpci.decode_tile(ti);
        let pal = if (mapping.palette_idx as usize) < 12 {
            &tileset.dpl.palettes[mapping.palette_idx as usize]
        } else {
            &tileset.dpl.palettes[0]
        };

        for py in 0..DPCI_TILE_DIM {
            for px in 0..DPCI_TILE_DIM {
                let sx = if mapping.flip_x { 7 - px } else { px };
                let sy = if mapping.flip_y { 7 - py } else { py };
                let ci = pixels[sy * DPCI_TILE_DIM + sx] as usize;

                let rgba = if ci == 0 {
                    Rgba([0, 0, 0, 0])
                } else {
                    let c = pal[ci];
                    Rgba([c.r, c.g, c.b, 255])
                };

                let ox = (tx + px) as u32;
                let oy = (ty + py) as u32;
                if ox < img.width() && oy < img.height() {
                    img.put_pixel(ox, oy, rgba);
                }
            }
        }
    }
}

fn build_metadata(tileset: &DungeonTileset, sw: usize, sh: usize) -> TilesetMetadata {
    TilesetMetadata {
        tileset_id: tileset.tileset_id,
        chunk_count: tileset.dpc.chunks.len(),
        sheet_width: sw,
        sheet_height: sh,
        chunk_size: CHUNK_PX,
        dma_rules: build_dma_rules(tileset),
        palettes: build_palette_list(tileset),
        animation: build_animation_meta(tileset),
    }
}

fn build_dma_rules(tileset: &DungeonTileset) -> DmaRules {
    let extract =
        |t: DmaType| -> Vec<[u8; 3]> { (0..=255u8).map(|n| tileset.dma.get(t, n)).collect() };
    DmaRules {
        wall: extract(DmaType::Wall),
        secondary: extract(DmaType::Secondary),
        floor: extract(DmaType::Floor),
    }
}

fn build_palette_list(tileset: &DungeonTileset) -> Vec<Vec<[u8; 3]>> {
    tileset
        .dpl
        .palettes
        .iter()
        .map(|pal| pal.iter().map(|c| [c.r, c.g, c.b]).collect())
        .collect()
}

fn build_animation_meta(tileset: &DungeonTileset) -> Option<AnimationMetadata> {
    let has_10 = tileset.dpla.has_animation_for_palette(10);
    let has_11 = tileset.dpla.has_animation_for_palette(11);
    if !has_10 && !has_11 {
        return None;
    }

    let extract_pal = |base: usize| -> Vec<ColourAnimation> {
        (0..16)
            .filter_map(|i| {
                let entry = &tileset.dpla.colours[base + i];
                if entry.num_frames == 0 {
                    return None;
                }
                Some(ColourAnimation {
                    colour_index: i,
                    duration_frames: entry.duration,
                    frames: entry.frames.iter().map(|c| [c.r, c.g, c.b]).collect(),
                })
            })
            .collect()
    };

    Some(AnimationMetadata {
        palette_10: extract_pal(0),
        palette_11: extract_pal(16),
    })
}
