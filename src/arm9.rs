use std::{
    collections::HashMap,
    io::{self, Cursor},
};

use crate::binary_utils;

/// Represents an overlay in a Nintendo DS ROM
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Overlay {
    pub id: u32,
    pub data: Vec<u8>,
    pub ram_address: u32,
    pub ram_size: u32,
    pub block_started_by_symbol_size: u32,
    pub static_init_start: u32,
    pub static_init_end: u32,
    pub file_id: u32,
    pub compressed_size: u32,
    pub flags: u8,
}

pub fn load_overlay_table(
    table_data: &[u8],
    file_callback: impl Fn(u32, u32) -> io::Result<Vec<u8>>,
    ids_to_load: Option<&[u32]>,
) -> io::Result<HashMap<u32, Overlay>> {
    let mut overlays = HashMap::new();
    let mut cursor = Cursor::new(table_data);

    let table_len = table_data.len();
    println!("Overlay table size: {} bytes", table_len);

    // Ensure we only process complete entries
    let entry_count = table_len / 32;
    println!("Overlay table contains {} complete entries", entry_count);

    for entry_idx in 0..entry_count {
        let entry_offset = entry_idx * 32;
        binary_utils::seek_to(&mut cursor, entry_offset as u64)?;

        let overlay_id = binary_utils::read_u32_le(&mut cursor)?;
        let ram_address = binary_utils::read_u32_le(&mut cursor)?;
        let ram_size = binary_utils::read_u32_le(&mut cursor)?;
        let block_started_by_symbol_size = binary_utils::read_u32_le(&mut cursor)?;
        let static_init_start = binary_utils::read_u32_le(&mut cursor)?;
        let static_init_end = binary_utils::read_u32_le(&mut cursor)?;
        let file_id = binary_utils::read_u32_le(&mut cursor)?;
        let compressed_size_flags = binary_utils::read_u32_le(&mut cursor)?;

        if let Some(ids) = ids_to_load {
            if !ids.contains(&overlay_id) {
                continue;
            }
        }

        match file_callback(overlay_id, file_id) {
            Ok(file_data) => {
                println!(
                    "  Successfully loaded overlay {} ({} bytes)",
                    overlay_id,
                    file_data.len()
                );
                overlays.insert(
                    overlay_id,
                    Overlay {
                        id: overlay_id,
                        data: file_data,
                        ram_address,
                        ram_size,
                        block_started_by_symbol_size,
                        static_init_start,
                        static_init_end,
                        file_id,
                        compressed_size: compressed_size_flags & 0xFFFFFF,
                        flags: (compressed_size_flags >> 24) as u8,
                    },
                );
            }
            Err(e) => {
                if ids_to_load.map_or(false, |ids| ids.contains(&overlay_id)) {
                    eprintln!(
                        "ERROR: Failed to load requested overlay {}: {}",
                        overlay_id, e
                    );
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Failed to load requested overlay {}: {}", overlay_id, e),
                    ));
                } else {
                    eprintln!(
                        "Warning: Failed to load optional overlay {}: {}",
                        overlay_id, e
                    );
                }
            }
        }
    }

    println!(
        "Loaded {} overlays: {:?}",
        overlays.len(),
        overlays.keys().collect::<Vec<_>>()
    );
    Ok(overlays)
}
