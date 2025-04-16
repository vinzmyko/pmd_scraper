use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use crate::arm9::{load_overlay_table, Overlay};
use crate::data::animation_info::{
    get_region_data, parse_animation_data, write_u32, AnimData, RegionData,
};
use crate::filesystem::{FileAllocationTable, FileNameTable};

/// Helper functions for reading values in little-endian order
fn read_u8(data: &[u8], offset: usize) -> u8 {
    data[offset]
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    let low = data[offset] as u16;
    let high = data[offset + 1] as u16;
    (high << 8) | low
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    let b0 = data[offset] as u32;
    let b1 = data[offset + 1] as u32;
    let b2 = data[offset + 2] as u32;
    let b3 = data[offset + 3] as u32;
    b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
}

/// Represents a Nintendo DS ROM
#[allow(dead_code)]
pub struct Rom {
    pub path: std::path::PathBuf,
    pub id_code: String,
    pub developer_code: String,
    pub game_title: String,
    pub data: Vec<u8>,
    pub arm9: Vec<u8>,
    pub arm9_ram_address: u32,
    pub arm9_entry_address: u32,
    pub arm9_size: u32,
    pub arm9_overlay_table: Vec<u8>,
    pub fat: FileAllocationTable,
    pub fnt: FileNameTable,
    pub region_data: RegionData,
    pub loaded_overlays: HashMap<u32, Overlay>,
}

impl Rom {
    /// Load a ROM from a file path
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();

        // Read full ROM data
        let mut file = File::open(&path_buf)?;
        let mut rom_data = Vec::new();
        file.read_to_end(&mut rom_data)?;

        // Read ROM header
        let rom_header = read_header(&rom_data)?;

        // Determine region
        let id_code = rom_header.game_code.clone();
        let region_data = get_region_data(&id_code).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported game region: {}", id_code),
            )
        })?;

        // Read ARM9 binary
        let arm9_offset = rom_header.arm9_rom_offset as usize;
        let arm9_size = rom_header.arm9_size as usize;
        let arm9 = rom_data[arm9_offset..arm9_offset + arm9_size].to_vec();

        // Extract the ARM9 overlay table using the correct header fields
        let arm9_ovt_offset = rom_header.arm9_overlay_table_offset as usize;
        let arm9_ovt_size = rom_header.arm9_overlay_table_size as usize;

        // Check bounds before slicing
        if arm9_ovt_offset + arm9_ovt_size > rom_data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ARM9 overlay table offset/size out of bounds: offset={}, size={}, data_len={}",
                    arm9_ovt_offset,
                    arm9_ovt_size,
                    rom_data.len()
                ),
            ));
        }

        let arm9_overlay_table =
            rom_data[arm9_ovt_offset..arm9_ovt_offset + arm9_ovt_size].to_vec();

        println!(
            "Loaded ARM9 overlay table: {} bytes",
            arm9_overlay_table.len()
        );

        // Read FAT and FNT
        let fat = FileAllocationTable::read_from_rom(
            &rom_data,
            rom_header.fat_offset,
            rom_header.fat_size,
        )?;

        let fnt = FileNameTable::read_from_rom(&rom_data, rom_header.fnt_offset)?;

        Ok(Rom {
            path: path_buf,
            id_code,
            developer_code: rom_header.maker_code,
            game_title: rom_header.game_title,
            data: rom_data,
            arm9,
            arm9_ram_address: rom_header.arm9_ram_address,
            arm9_entry_address: rom_header.arm9_entry_address,
            arm9_size: rom_header.arm9_size,
            arm9_overlay_table,
            fat,
            fnt,
            region_data,
            loaded_overlays: HashMap::new(),
        })
    }

    /// Load specific overlays from the ROM
    pub fn load_arm9_overlays(
        &mut self,
        ids_to_load: &[u32],
    ) -> io::Result<&HashMap<u32, Overlay>> {
        println!("Loading ARM9 overlays: {:?}", ids_to_load);
        println!(
            "Current loaded_overlays: {:?}",
            self.loaded_overlays.keys().collect::<Vec<_>>()
        );
        println!(
            "Overlay table size: {} bytes",
            self.arm9_overlay_table.len()
        );

        // Create callback to load overlay files from FAT
        let rom_data = self.read_rom_data()?;
        let fat = &self.fat;

        let file_callback = move |ov_id: u32, file_id: u32| -> io::Result<Vec<u8>> {
            println!(
                "Callback invoked for overlay ID: {}, file ID: {}",
                ov_id, file_id
            );
            if let Some(data) = fat.get_file_data(file_id as usize, &rom_data) {
                println!("  Successfully loaded file data: {} bytes", data.len());
                Ok(data.to_vec())
            } else {
                let err = io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Failed to get file data for overlay file ID {}", file_id),
                );
                println!("  Error: {}", err);
                Err(err)
            }
        };

        // Load overlays using the improved error handling
        let overlays =
            load_overlay_table(&self.arm9_overlay_table, file_callback, Some(ids_to_load))?;

        println!(
            "load_overlay_table returned {} overlays: {:?}",
            overlays.len(),
            overlays.keys().collect::<Vec<_>>()
        );

        for (id, overlay) in overlays {
            self.loaded_overlays.insert(id, overlay);
        }

        Ok(&self.loaded_overlays)
    }

    /// Read the entire ROM data
    fn read_rom_data(&self) -> io::Result<Vec<u8>> {
        let mut file = File::open(&self.path)?;
        let mut rom_data = Vec::new();
        file.read_to_end(&mut rom_data)?;
        Ok(rom_data)
    }

    /// Extract animation data from overlay 10
    pub fn extract_animation_data(&mut self) -> Result<AnimData, String> {
        println!("Starting extract_animation_data");
        println!(
            "Current loaded_overlays: {:?}",
            self.loaded_overlays.keys().collect::<Vec<_>>()
        );

        // Make sure overlay 10 is loaded
        if !self.loaded_overlays.contains_key(&10) {
            println!("Overlay 10 not found, trying to load it now");
            match self.load_arm9_overlays(&[10]) {
                Ok(_) => {
                    println!("Successfully loaded overlay 10");
                    println!(
                        "loaded_overlays now: {:?}",
                        self.loaded_overlays.keys().collect::<Vec<_>>()
                    );
                }
                Err(e) => {
                    return Err(format!("Failed to load overlay 10: {}", e));
                }
            }
        } else {
            println!("Overlay 10 is already loaded");
        }

        // Get overlay 10
        let overlay10 = self.loaded_overlays.get(&10).ok_or_else(|| {
            println!("ERROR: Overlay 10 still not found in loaded_overlays after loading attempt");
            println!(
                "Current loaded_overlays: {:?}",
                self.loaded_overlays.keys().collect::<Vec<_>>()
            );
            "Overlay 10 not loaded".to_string()
        })?;

        println!(
            "Successfully found overlay 10 ({} bytes)",
            overlay10.data.len()
        );

        // Get the start address for animation data
        let start_table = self.region_data.start_table as usize;

        // Check if start_table is valid
        if start_table >= overlay10.data.len() {
            return Err(format!(
                "Start table offset 0x{:X} is out of bounds for overlay 10 (size: 0x{:X})",
                start_table,
                overlay10.data.len()
            ));
        }

        // Create header as the patch would
        let mut header = vec![0u8; 5 * 4];
        write_u32(&mut header, 5 * 4, 0); // Header size
        write_u32(&mut header, 5 * 4 + 52, 4); // Offset to trap table
        write_u32(&mut header, 5 * 4 + 52 + 5600, 8); // Offset to item table
        write_u32(&mut header, 5 * 4 + 52 + 5600 + 13512, 12); // Offset to move table
        write_u32(&mut header, 5 * 4 + 52 + 5600 + 13512 + 19600, 16); // Offset to special move table

        // Extract the animation data (0x14560 bytes as in the patch)
        let anim_data_size = usize::min(overlay10.data.len() - start_table, 0x14560);
        let animation_data = [
            &header[..],
            &overlay10.data[start_table..start_table + anim_data_size],
        ]
        .concat();

        // Parse the animation data
        parse_animation_data(&animation_data)
    }
}

/// ROM header information
#[derive(Debug)]
pub struct RomHeader {
    pub game_title: String,
    pub game_code: String,
    pub maker_code: String,
    pub arm9_rom_offset: u32,
    pub arm9_entry_address: u32,
    pub arm9_ram_address: u32,
    pub arm9_size: u32,
    pub arm9_overlay_table_offset: u32,
    pub arm9_overlay_table_size: u32,
    pub fnt_offset: u32,
    pub fnt_size: u32,
    pub fat_offset: u32,
    pub fat_size: u32,
    pub unit_code: u8,
    pub nds_region: u8,
    pub rom_version: u8,
    pub device_capacity: u8,
    pub encryption_seed: u8,
}

/// Read the ROM header from a file
fn read_header(rom_data: &[u8]) -> io::Result<RomHeader> {
    // Read game title (12 bytes)
    let title_offset = 0x000;
    let mut title_buffer = [0u8; 12];
    title_buffer.copy_from_slice(&rom_data[title_offset..title_offset + 12]);
    let game_title = String::from_utf8_lossy(&title_buffer)
        .trim_end_matches('\0')
        .to_string();

    // Read game code (4 bytes)
    let game_code_offset = 0x00C;
    let mut game_code_buffer = [0u8; 4];
    game_code_buffer.copy_from_slice(&rom_data[game_code_offset..game_code_offset + 4]);
    let game_code = String::from_utf8_lossy(&game_code_buffer).to_string();

    // Read maker code (2 bytes)
    let maker_code_offset = 0x010;
    let mut maker_code_buffer = [0u8; 2];
    maker_code_buffer.copy_from_slice(&rom_data[maker_code_offset..maker_code_offset + 2]);
    let maker_code = String::from_utf8_lossy(&maker_code_buffer).to_string();

    // Read unit code (1 byte)
    let unit_code = rom_data[0x012];

    // Read NDS region (1 byte)
    let nds_region = rom_data[0x01D];

    // Read ROM version (1 byte)
    let rom_version = rom_data[0x01E];

    // Read device capacity (1 byte)
    let device_capacity = rom_data[0x014];

    // Read encryption seed (1 byte)
    let encryption_seed = rom_data[0x013];

    // Read ARM9 ROM offset (4 bytes)
    let arm9_rom_offset = read_u32(rom_data, 0x020);

    // Read ARM9 entry address (4 bytes)
    let arm9_entry_address = read_u32(rom_data, 0x024);

    // Read ARM9 RAM address (4 bytes)
    let arm9_ram_address = read_u32(rom_data, 0x028);

    // Read ARM9 size (4 bytes)
    let arm9_size = read_u32(rom_data, 0x02C);

    // Read ARM9 overlay table offset and size directly from header
    let arm9_overlay_table_offset = read_u32(rom_data, 0x050);
    let arm9_overlay_table_size = read_u32(rom_data, 0x054);

    // Read FNT offset (4 bytes)
    let fnt_offset = read_u32(rom_data, 0x040);

    // Read FNT size (4 bytes)
    let fnt_size = read_u32(rom_data, 0x044);

    // Read FAT offset (4 bytes)
    let fat_offset = read_u32(rom_data, 0x048);

    // Read FAT size (4 bytes)
    let fat_size = read_u32(rom_data, 0x04C);

    Ok(RomHeader {
        game_title,
        game_code,
        maker_code,
        arm9_rom_offset,
        arm9_entry_address,
        arm9_ram_address,
        arm9_size,
        arm9_overlay_table_offset,
        arm9_overlay_table_size,
        fnt_offset,
        fnt_size,
        fat_offset,
        fat_size,
        unit_code,
        nds_region,
        rom_version,
        device_capacity,
        encryption_seed,
    })
}
