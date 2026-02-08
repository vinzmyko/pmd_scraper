//! # Dungeon Palette Chunks
//!
//! A single dungeon tile is 24x24 pixels, but uses the chunks of 8x8 pixel tiles. A 'Chunk' is a
//! 3x3 grid of tiles.
//!
//! Contains the instrucutions on how to build the 24x24 chunks.

use std::io;

/// A 24x24 tile is made of 9 (3x3) chunks
pub const DPC_TILES_PER_CHUNK: usize = 9; // 3×3

#[derive(Clone, Copy, Debug, Default)]
pub struct TileMapping {
    pub tile_index: u16,
    pub flip_x: bool,
    pub flip_y: bool,
    pub palette_idx: u8,
}

impl TileMapping {
    pub fn from_u16(val: u16) -> Self {
        TileMapping {
            tile_index: val & 0x3FF,
            flip_x: (val & 0x400) != 0,
            flip_y: (val & 0x800) != 0,
            palette_idx: ((val >> 12) & 0xF) as u8,
        }
    }
}

pub struct Dpc {
    pub chunks: Vec<[TileMapping; DPC_TILES_PER_CHUNK]>,
}

impl Dpc {
    pub fn from_bytes(data: &[u8]) -> Result<Self, io::Error> {
        let bytes_per_chunk = DPC_TILES_PER_CHUNK * 2; // 18 bytes
        if data.len() % bytes_per_chunk != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "DPC data length {} not divisible by {}",
                    data.len(),
                    bytes_per_chunk
                ),
            ));
        }
        let chunks = data
            .chunks_exact(bytes_per_chunk)
            .map(|chunk_data| {
                let mut mappings = [TileMapping::default(); DPC_TILES_PER_CHUNK];
                for i in 0..DPC_TILES_PER_CHUNK {
                    let val = u16::from_le_bytes([chunk_data[i * 2], chunk_data[i * 2 + 1]]);
                    mappings[i] = TileMapping::from_u16(val);
                }
                mappings
            })
            .collect();
        Ok(Dpc { chunks })
    }
}
