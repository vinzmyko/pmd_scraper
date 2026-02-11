//! # Dungeon Map Assembly
//!
//! Handles the autotiling logic of the dungeon.
//! The game looks at a specific grid cell and its 8 neighbours to determine which graphic to draw.
//!
//! - Input: Terrain type + neighbour bitmask
//! - Output: Chunk ID (index into DPC)
//!
//! There are 3 variations for every neighbour config to add visual variety.

use std::io;

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum DmaType {
    Wall = 0,
    /// Water, Lava, Chasm etc.
    Secondary = 1,
    Floor = 2,
}

pub struct Dma {
    pub chunk_mappings: Vec<u8>,
}

impl Dma {
    pub fn from_bytes(data: &[u8]) -> Result<Self, io::Error> {
        if data.len() < 0x930 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("DMA data too short: {} < {}", data.len(), 0x930),
            ));
        }
        Ok(Dma {
            chunk_mappings: data.to_vec(),
        })
    }

    /// Returns 3 chunk variation indices for a tile type + neighbor config
    pub fn get(&self, tile_type: DmaType, neighbors: u8) -> [u8; 3] {
        let base = (tile_type as usize) * 256 * 3 + (neighbors as usize) * 3;
        [
            self.chunk_mappings[base],
            self.chunk_mappings[base + 1],
            self.chunk_mappings[base + 2],
        ]
    }
}
