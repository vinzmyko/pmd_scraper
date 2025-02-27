use crate::formats::at4px::At4pxContainer;
use crate::formats::containers::ContainerHandler;
use image::RgbaImage;
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

        // Get container size and deserialise in one step
        let (container_size, _) =
            At4pxContainer::get_container_size_and_deserialise(&data[KAO_IMG_PAL_SIZE..])
                .map_err(|e| format!("Failed to parse AT4PX container: {}", e))?;

        // Avoid cloning the data by using a slice
        let compressed_data = data[KAO_IMG_PAL_SIZE..KAO_IMG_PAL_SIZE + container_size].to_vec();
        let _original_size = KAO_IMG_PAL_SIZE + container_size;

        Ok(Portrait {
            palette,
            compressed_data,
            _original_size,
        })
    }

    pub fn to_rgba_image(&self) -> Result<RgbaImage, String> {
        // Create AT4PX container from compressed data
        let container = At4pxContainer::deserialise(&self.compressed_data)
            .map_err(|e| format!("Failed to create AT4PX container: {}", e))?;

        // Decompress container as image data
        let decompressed = container.decompress()?;

        const IMG_DIM: u32 = 40;
        const TILE_DIM: usize = 8;
        const GRID_DIM: usize = 5;
        const PIXELS_PER_TILE: usize = TILE_DIM * TILE_DIM;
        const TOTAL_PIXELS: usize = (IMG_DIM * IMG_DIM) as usize;

        // Buffer that holds the entire rgba image, each pixel contains rgba (4)
        let mut image_buffer = vec![0u8; (IMG_DIM * IMG_DIM * 4) as usize];

        // Pre-calculates top left corner position
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

                // Calculate which tile this pixel belongs to
                let tile_id = idx / PIXELS_PER_TILE;

                if tile_id >= tile_positions.len() {
                    continue; // Protect against out-of-bounds
                }

                // Get pre-calculated tile position
                let (tile_x, tile_y) = tile_positions[tile_id];

                // Calculate position within the current tile
                let idx_in_tile = idx - (PIXELS_PER_TILE * tile_id);
                let in_tile_x = (idx_in_tile % TILE_DIM) as u32;
                let in_tile_y = (idx_in_tile / TILE_DIM) as u32;

                // Calculate final pixel position
                let x = tile_x + in_tile_x;
                let y = tile_y + in_tile_y;

                if x >= IMG_DIM || y >= IMG_DIM {
                    continue; // Protect against out-of-bounds
                }

                // Get color index based on which nibble we're processing
                let color_idx = if i == 0 { color_idx1 } else { color_idx2 } as usize;

                if color_idx >= self.palette.len() {
                    continue; // Protect against out-of-bounds
                }

                // Calculate position in the RGBA buffer (4 bytes per pixel)
                let buffer_pos = ((y * IMG_DIM + x) * 4) as usize;

                // Copy color data to buffer
                let color = &self.palette[color_idx];
                image_buffer[buffer_pos] = color[0];
                image_buffer[buffer_pos + 1] = color[1];
                image_buffer[buffer_pos + 2] = color[2];
                image_buffer[buffer_pos + 3] = 255; // Alpha
            }
        }

        // Create image from buffer
        RgbaImage::from_raw(IMG_DIM, IMG_DIM, image_buffer)
            .ok_or_else(|| "Failed to create image from buffer".to_string())
    }
}

/// Represents the entire KAO file containing multiple portraits
pub const KAO_PORTRAITS_PER_POKEMON: usize = 40;
pub const KAO_PORTRAIT_POINTER_SIZE: usize = 4;
pub const KAO_IMG_PAL_SIZE: usize = 48;
pub const _KAO_IMG_DIM: usize = 40;
pub const _KAO_TILE_DIM: usize = 8;
pub const _KAO_META_DIM: usize = 5;
pub const KAO_FIRST_TOC_OFFSET: usize = 160;

#[derive(Debug)]
pub struct KaoFile {
    data: Vec<u8>,
    toc_start_offset: usize,
    pokemon_count: usize,
}

impl KaoFile {
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, String> {
        // First 160 bytes are padding
        let toc_start_offset = KAO_FIRST_TOC_OFFSET;

        if data.len() < toc_start_offset + 4 {
            return Err("Data too short for KAO file".to_string());
        }

        // Read first portrait_pointer to determine TOC length
        let first_portrait_portrait_pointer = i32::from_le_bytes(
            data[toc_start_offset..toc_start_offset + 4]
                .try_into()
                .unwrap(),
        );

        let toc_size_bytes = (first_portrait_portrait_pointer as usize) - toc_start_offset;
        let pokemon_entry_size = KAO_PORTRAITS_PER_POKEMON * KAO_PORTRAIT_POINTER_SIZE;
        let pokemon_count = toc_size_bytes / pokemon_entry_size;

        Ok(KaoFile {
            data,
            toc_start_offset,
            pokemon_count,
        })
    }

    pub fn get_portrait(&self, index: usize, subindex: usize) -> Result<Option<Portrait>, String> {
        if index >= self.pokemon_count {
            return Err(format!(
                "Portrait index {} out of bounds (max {})",
                index, self.pokemon_count
            ));
        }
        if subindex >= KAO_PORTRAITS_PER_POKEMON {
            return Err(format!(
                "Subindex {} out of bounds (max {})",
                subindex, KAO_PORTRAITS_PER_POKEMON
            ));
        }

        // Calculate TOC entry position
        let entry_pos = self.toc_start_offset
            + (index * KAO_PORTRAITS_PER_POKEMON * KAO_PORTRAIT_POINTER_SIZE)
            + (subindex * KAO_PORTRAIT_POINTER_SIZE);

        if entry_pos + 4 > self.data.len() {
            return Err("Invalid TOC entry position".to_string());
        }

        // Read pointer
        let portrait_pointer =
            i32::from_le_bytes(self.data[entry_pos..entry_pos + 4].try_into().unwrap());

        // Negative pointer means no portrait at this position
        if portrait_pointer < 0 {
            return Ok(None);
        }

        let portrait_pos = portrait_pointer as usize;
        if portrait_pos >= self.data.len() {
            return Err("Invalid portrait portrait_pointer".to_string());
        }

        Portrait::from_bytes(&self.data[portrait_pos..]).map(Some)
    }
}
