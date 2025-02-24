use crate::formats::at4px::At4pxContainer;
use crate::formats::containers::ContainerHandler;
use crate::formats::containers::{KAO_IMG_PAL_SIZE, SUBENTRIES, SUBENTRY_LEN};
use image::{Rgba, RgbaImage};
use std::convert::TryInto;

/// Represents a single portrait image from the KAO file
#[derive(Clone, Debug)]
pub struct Portrait {
    palette: Vec<[u8; 3]>,    // RGB colors
    compressed_data: Vec<u8>, // AT4PX compressed data
    _original_size: usize,    // Size of original data
}

impl Portrait {
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < KAO_IMG_PAL_SIZE {
            return Err("Data too short for portrait".to_string());
        }

        // Read palette - 16 RGB colors
        let mut palette = Vec::with_capacity(16);
        for i in 0..16 {
            let offset = i * 3;
            palette.push([data[offset], data[offset + 1], data[offset + 2]]);
        }

        // Get container size and deserialize in one step
        let (container_size, _) =
            At4pxContainer::get_container_size_and_deserialize(&data[KAO_IMG_PAL_SIZE..])
                .map_err(|e| format!("Failed to parse AT4PX container: {}", e))?;

        let compressed_data = data[KAO_IMG_PAL_SIZE..KAO_IMG_PAL_SIZE + container_size].to_vec();
        let _original_size = KAO_IMG_PAL_SIZE + container_size;

        Ok(Portrait {
            palette,
            compressed_data,
            _original_size,
        })
    }

    pub fn to_rgba_image(&self) -> Result<RgbaImage, String> {
        // Create AT4PX container and decompress
        let container = At4pxContainer::deserialize(&self.compressed_data)
            .map_err(|e| format!("Failed to create AT4PX container: {}", e))?;

        let decompressed = container.decompress()?;

        const IMG_DIM: u32 = 40;
        const TILE_DIM: usize = 8;
        const GRID_DIM: usize = 5;
        const PIXELS_PER_TILE: usize = TILE_DIM * TILE_DIM;
        const TOTAL_PIXELS: usize = (IMG_DIM * IMG_DIM) as usize;

        // Create image buffer
        let mut image = RgbaImage::new(IMG_DIM, IMG_DIM);

        // Pre-calculate tile positions to avoid repeated calculations
        let mut tile_positions = Vec::with_capacity(GRID_DIM * GRID_DIM);
        for tile_id in 0..(GRID_DIM * GRID_DIM) {
            let tile_x = (tile_id % GRID_DIM) as u32;
            let tile_y = (tile_id / GRID_DIM) as u32;
            tile_positions.push((tile_x * TILE_DIM as u32, tile_y * TILE_DIM as u32));
        }

        let expected_pixels = TOTAL_PIXELS;
        let actual_pixels = decompressed.len() * 2; // Each byte has 2 pixels
        let pixel_count = std::cmp::min(expected_pixels, actual_pixels);

        // Process decompressed data (each byte contains two 4-bit pixels)
        for byte_idx in 0..(pixel_count / 2) {
            let byte = decompressed[byte_idx];

            // Extract both 4-bit values from the byte
            let color_idx2 = (byte >> 4) & 0xF; // High nibble
            let color_idx1 = byte & 0xF; // Low nibble

            // Handle two pixels
            for i in 0..2 {
                let idx = byte_idx * 2 + i;
                // We can skip the bounds check since we've pre-calculated pixel_count

                // Calculate which tile this pixel belongs to
                let tile_id = idx / PIXELS_PER_TILE;

                // Get pre-calculated tile position
                let (tile_x, tile_y) = tile_positions[tile_id];

                // Calculate position within the current tile
                let idx_in_tile = idx - (PIXELS_PER_TILE * tile_id);
                let in_tile_x = (idx_in_tile % TILE_DIM) as u32;
                let in_tile_y = (idx_in_tile / TILE_DIM) as u32;

                // Calculate final pixel position
                let x = tile_x + in_tile_x;
                let y = tile_y + in_tile_y;

                // Get color index based on which nibble we're processing
                let color_idx = if i == 0 { color_idx1 } else { color_idx2 };

                // Place the pixel - no need to check if x/y are within bounds
                let color = &self.palette[color_idx as usize];
                image.put_pixel(x, y, Rgba([color[0], color[1], color[2], 255]));
            }
        }

        Ok(image)
    }
}

/// Represents the entire KAO file containing multiple portraits
#[derive(Debug)]
pub struct KaoFile {
    data: Vec<u8>,
    first_toc: usize,
    toc_len: usize,
}

impl KaoFile {
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, String> {
        // First 160 bytes are padding, followed by TOC
        let first_toc = SUBENTRIES * SUBENTRY_LEN;

        if data.len() < first_toc + 4 {
            return Err("Data too short for KAO file".to_string());
        }

        // Read first pointer to determine TOC length
        let first_pointer = i32::from_le_bytes(data[first_toc..first_toc + 4].try_into().unwrap());
        let toc_len = ((first_pointer as usize) - first_toc) / (SUBENTRIES * SUBENTRY_LEN);

        // Take ownership of data instead of copying it
        Ok(KaoFile {
            data,
            first_toc,
            toc_len,
        })
    }

    pub fn get_portrait(&self, index: usize, subindex: usize) -> Result<Option<Portrait>, String> {
        if index >= self.toc_len {
            return Err(format!(
                "Portrait index {} out of bounds (max {})",
                index, self.toc_len
            ));
        }
        if subindex >= SUBENTRIES {
            return Err(format!(
                "Subindex {} out of bounds (max {})",
                subindex, SUBENTRIES
            ));
        }

        // Calculate TOC entry position
        let entry_pos =
            self.first_toc + (index * SUBENTRIES * SUBENTRY_LEN) + (subindex * SUBENTRY_LEN);

        if entry_pos + 4 > self.data.len() {
            return Err("Invalid TOC entry position".to_string());
        }

        // Read pointer
        let pointer = i32::from_le_bytes(self.data[entry_pos..entry_pos + 4].try_into().unwrap());

        // Negative pointer means no portrait at this position
        if pointer < 0 {
            return Ok(None);
        }

        let portrait_pos = pointer as usize;
        if portrait_pos >= self.data.len() {
            return Err("Invalid portrait pointer".to_string());
        }

        Portrait::from_bytes(&self.data[portrait_pos..]).map(Some)
    }
}
