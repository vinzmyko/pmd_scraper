use crate::formats::at4px::At4pxContainer;
use crate::formats::containers::ContainerHandler;
use crate::formats::containers::{KAO_IMG_PAL_SIZE, SUBENTRIES, SUBENTRY_LEN};
use image::{ImageBuffer, Rgba, RgbaImage};
use std::convert::TryInto;

/// Represents a single portrait image from the KAO file
#[derive(Clone, Debug)]
pub struct Portrait {
    palette: Vec<[u8; 3]>,    // RGB colors
    compressed_data: Vec<u8>, // AT4PX compressed data
    pub original_size: usize, // Size of original data
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

        // Get container size before trying to parse
        let container_size = At4pxContainer::get_container_size(&data[KAO_IMG_PAL_SIZE..])
            .map_err(|e| format!("Failed to get container size: {}", e))?;

        // The rest is AT4PX compressed image data
        let container = At4pxContainer::deserialize(&data[KAO_IMG_PAL_SIZE..])
            .map_err(|e| format!("Failed to parse AT4PX container: {}", e))?;

        let original_size = KAO_IMG_PAL_SIZE + container_size;
        let compressed_data = data[KAO_IMG_PAL_SIZE..KAO_IMG_PAL_SIZE + container_size].to_vec();

        Ok(Portrait {
            palette,
            compressed_data,
            original_size,
        })
    }

    pub fn to_rgba_image(&self) -> Result<RgbaImage, String> {
        // Create AT4PX container and decompress
        let container = At4pxContainer::deserialize(&self.compressed_data)
            .map_err(|e| format!("Failed to create AT4PX container: {}", e))?;

        let decompressed = container.decompress()?;

        // Image dimensions
        let img_dim = 40; // 5 tiles * 8 pixels = 40x40 image
        let mut image = ImageBuffer::new(img_dim, img_dim);

        // Process decompressed data (each byte contains two 4-bit pixels)
        for (byte_idx, &byte) in decompressed.iter().enumerate() {
            // Extract both 4-bit values from the byte
            let color_idx2 = (byte >> 4) & 0xF; // Store high nibble (will be used second)
            let color_idx1 = byte & 0xF;        // Store low nibble (will be used first)

            // Process each nibble (4-bit value) in the byte
            for i in 0..2 {
                let idx = byte_idx * 2 + i; // Each byte holds 2 pixels

                // Calculate which 8x8 tile this pixel belongs to
                let tile_id = idx / 64; // Integer division gives us tile number
                let tile_x = tile_id % 5; // X position in tile grid (0-4)
                let tile_y = tile_id / 5; // Y position in tile grid (0-4)

                // Calculate position within the current 8x8 tile
                let idx_in_tile = idx - (64 * tile_id); // Position within current tile (0-63)
                let in_tile_x = idx_in_tile % 8; // X position within tile (0-7)
                let in_tile_y = idx_in_tile / 8; // Y position within tile (0-7)

                // Calculate final pixel position
                let x = (tile_x * 8) + in_tile_x;
                let y = (tile_y * 8) + in_tile_y;

                // Get color index based on which nibble we're processing
                let color_idx = if i == 0 { color_idx1 } else { color_idx2 };

                // Place the pixel if it's within image bounds
                if x < img_dim.try_into().unwrap() && y < img_dim.try_into().unwrap() {
                    let color = &self.palette[color_idx as usize];
                    image.put_pixel(
                        x as u32,
                        y as u32,
                        Rgba([color[0], color[1], color[2], 255]),
                    );
                }
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

        // Read portrait data
        Portrait::from_bytes(&self.data[portrait_pos..]).map(Some)
    }

    pub fn len(&self) -> usize {
        self.toc_len
    }
}
