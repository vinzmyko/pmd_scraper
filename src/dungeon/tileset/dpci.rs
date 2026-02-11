//! # Dungeon Palette Chunk Indices
//!
//! Contains the raw, uncoloured pixel data.
//!
//! There are 8x8 tiles stored in 4bpp format.

use std::io;

pub const DPCI_TILE_DIM: usize = 8;
pub const DPCI_BYTES_PER_TILE: usize = 32; // 8×8 pixels at 4bpp

pub struct Dpci {
    pub tiles: Vec<[u8; DPCI_BYTES_PER_TILE]>,
}

impl Dpci {
    pub fn from_bytes(data: &[u8]) -> Result<Self, io::Error> {
        if data.len() % DPCI_BYTES_PER_TILE != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "DPCI data length {} not divisible by {}",
                    data.len(),
                    DPCI_BYTES_PER_TILE
                ),
            ));
        }
        let tiles = data
            .chunks_exact(DPCI_BYTES_PER_TILE)
            .map(|chunk| {
                let mut tile = [0u8; DPCI_BYTES_PER_TILE];
                tile.copy_from_slice(chunk);
                tile
            })
            .collect();
        Ok(Dpci { tiles })
    }

    /// Decode a 4bpp tile into 8×8 palette indices (0-15)
    pub fn decode_tile(&self, tile_idx: usize) -> [u8; 64] {
        let mut pixels = [0u8; 64];
        let tile = &self.tiles[tile_idx];
        for i in 0..32 {
            pixels[i * 2] = tile[i] & 0x0F;
            pixels[i * 2 + 1] = (tile[i] >> 4) & 0x0F;
        }
        pixels
    }
}
