use std::collections::HashMap;
use std::io::{self};

/// Represents an overlay in a Nintendo DS ROM
#[derive(Debug, Clone)]
pub struct Overlay {
    pub id: u32,
    pub data: Vec<u8>,
    pub ram_address: u32,
    pub ram_size: u32,
    pub bss_size: u32,
    pub static_init_start: u32,
    pub static_init_end: u32,
    pub file_id: u32,
    pub compressed_size: u32,
    pub flags: u8,
}

/// Helper functions for reading values in little-endian order
fn read_u32(data: &[u8], offset: usize) -> u32 {
    let b0 = data[offset] as u32;
    let b1 = data[offset + 1] as u32;
    let b2 = data[offset + 2] as u32;
    let b3 = data[offset + 3] as u32;
    b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
}

// Replace the loop in load_overlay_table with bounded iteration:
pub fn load_overlay_table(
    table_data: &[u8],
    file_callback: impl Fn(u32, u32) -> io::Result<Vec<u8>>,
    ids_to_load: Option<&[u32]>,
) -> io::Result<HashMap<u32, Overlay>> {
    let mut overlays = HashMap::new();

    let table_len = table_data.len();
    println!("Overlay table size: {} bytes", table_len);

    // Ensure we only process complete entries
    let entry_count = table_len / 32;
    println!("Overlay table contains {} complete entries", entry_count);

    for entry_idx in 0..entry_count {
        let i = entry_idx * 32;

        // Parse overlay entry
        let ov_id = read_u32(table_data, i);
        let ram_addr = read_u32(table_data, i + 4);
        let ram_size = read_u32(table_data, i + 8);
        let bss_size = read_u32(table_data, i + 12);
        let static_init_start = read_u32(table_data, i + 16);
        let static_init_end = read_u32(table_data, i + 20);
        let file_id = read_u32(table_data, i + 24);
        let compressed_size_flags = read_u32(table_data, i + 28);

        println!(
            "Entry {}/{}: ID={}, file_id={}, offset={}",
            entry_idx + 1,
            entry_count,
            ov_id,
            file_id,
            i
        );

        // Skip if not in ids_to_load
        if let Some(ids) = ids_to_load {
            if !ids.contains(&ov_id) {
                println!("  Skipping overlay {} (not requested)", ov_id);
                continue;
            }
        }

        // Load file data with enhanced error handling
        println!(
            "  Attempting to load overlay {} (file_id={})",
            ov_id, file_id
        );
        match file_callback(ov_id, file_id) {
            Ok(file_data) => {
                println!(
                    "  Successfully loaded overlay {} ({} bytes)",
                    ov_id,
                    file_data.len()
                );
                overlays.insert(
                    ov_id,
                    Overlay {
                        id: ov_id,
                        data: file_data,
                        ram_address: ram_addr,
                        ram_size,
                        bss_size,
                        static_init_start,
                        static_init_end,
                        file_id,
                        compressed_size: compressed_size_flags & 0xFFFFFF,
                        flags: (compressed_size_flags >> 24) as u8,
                    },
                );
            }
            Err(e) => {
                // If specifically requested, return an error
                if ids_to_load.map_or(false, |ids| ids.contains(&ov_id)) {
                    eprintln!("ERROR: Failed to load requested overlay {}: {}", ov_id, e);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Failed to load requested overlay {}: {}", ov_id, e),
                    ));
                } else {
                    eprintln!("Warning: Failed to load optional overlay {}: {}", ov_id, e);
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
