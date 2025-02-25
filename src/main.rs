mod filesystem;
mod formats;
mod rom;

use crate::formats::portrait::KaoFile;

use filesystem::{FileAllocationTable, FileNameTable};
use rom::{read_header, RomHeader};
use std::fs::File;
use std::fs;
use std::path::PathBuf;
use std::io::Read;

fn main() {
    let rom_path = PathBuf::from("../../ROMs/pmd_eos_us.nds");
    let output_dir = PathBuf::from("./output/FONT");

    // Create output directory if it doesn't exist
    if !output_dir.exists() {
        if let Err(e) = fs::create_dir_all(&output_dir) {
            eprintln!("Failed to create output directory: {}", e);
            return;
        }
    }

    println!("ROM Path: {:?}", &rom_path);
    println!("Output Dir: {:?}", output_dir);

    let header = read_header(&rom_path);
    println!("ROM Header: {:#?}", header);

    match extract_portraits(&rom_path, &output_dir) {
        Ok(_) => println!("Successfully extracted portraits!"),
        Err(e) => eprintln!("Error extracting portraits: {}", e),
    }
}

fn read_rom_header(rom_path: &PathBuf) -> Result<RomHeader, String> {
    // Efficient header reading by only opening the file once and reading exactly what we need
    let mut file = File::open(rom_path)
        .map_err(|e| format!("Failed to open ROM file: {}", e))?;
    
    let mut buffer = [0u8; 0x50]; // Header size
    file.read_exact(&mut buffer)
        .map_err(|e| format!("Failed to read ROM header: {}", e))?;
    
    // Create header directly from buffer
    let title_buffer = &buffer[0x00..0x0C];
    let game_code_buffer = &buffer[0x0C..0x10];
    let maker_code_buffer = &buffer[0x10..0x12];
    
    let header = RomHeader {
        game_title: String::from_utf8_lossy(title_buffer)
            .trim_end_matches('\0')
            .to_string(),
        game_code: String::from_utf8_lossy(game_code_buffer).to_string(),
        maker_code: String::from_utf8_lossy(maker_code_buffer).to_string(),
        unit_code: buffer[0x12],
        encryption_seed: buffer[0x13],
        device_capacity: buffer[0x14],
        nds_region: buffer[0x1D],
        rom_version: buffer[0x1E],
        arm9_rom_offset: u32::from_le_bytes(buffer[0x20..0x24].try_into().unwrap()),
        arm9_entry_address: u32::from_le_bytes(buffer[0x24..0x28].try_into().unwrap()),
        arm9_ram_address: u32::from_le_bytes(buffer[0x28..0x2C].try_into().unwrap()),
        arm9_size: u32::from_le_bytes(buffer[0x2C..0x30].try_into().unwrap()),
        fnt_offset: u32::from_le_bytes(buffer[0x40..0x44].try_into().unwrap()),
        fnt_size: u32::from_le_bytes(buffer[0x44..0x48].try_into().unwrap()),
        fat_offset: u32::from_le_bytes(buffer[0x48..0x4C].try_into().unwrap()),
        fat_size: u32::from_le_bytes(buffer[0x4C..0x50].try_into().unwrap()),
    };
    
    Ok(header)
}

fn extract_portraits(rom_path: &PathBuf, output_dir: &PathBuf) -> Result<(), String> {
    // Read ROM data once - this is a key optimization we want to keep
    let rom_data = fs::read(rom_path).map_err(|e| format!("Failed to read ROM file: {}", e))?;

    let header = read_header(rom_path);

    let fat = FileAllocationTable::read_from_rom(&rom_data, header.fat_offset, header.fat_size)
        .map_err(|e| format!("Failed to read FAT: {}", e))?;

    let fnt = FileNameTable::read_from_rom(&rom_data, header.fnt_offset)
        .map_err(|e| format!("Failed to read FNT: {}", e))?;

    let kao_id = fnt.get_file_id("FONT/kaomado.kao").ok_or("Could not find kaomado.kao")?;
    
    let kao_data = fat.get_file_data(kao_id as usize, &rom_data)
        .ok_or("Could not read KAO file data")?;

    println!("Found KAO file: FONT/kaomado.kao (ID: {})", kao_id);
    println!("KAO file size: {} bytes", kao_data.len());

    let kao_file = KaoFile::from_bytes(kao_data.to_vec())?;

    // Create output directory if needed
    fs::create_dir_all(output_dir).map_err(|e| format!("Failed to create output directory: {}", e))?;

    // For your use case: extract all portraits that exist
    println!("Extracting portraits to build sprite atlases...");
    let mut extracted_count = 0;

    // Process portraits in batches
    for pokemon_id in 0..= 600 {
        if let Ok(Some(portrait)) = kao_file.get_portrait(pokemon_id, 0) {
            let output_path = output_dir.join(format!("pokemon_{:03}.png", pokemon_id));
            
            match portrait.to_rgba_image() {
                Ok(image) => {
                    if let Err(e) = image.save(&output_path) {
                        println!("Warning: Failed to save portrait {}: {}", pokemon_id, e);
                    } else {
                        extracted_count += 1;
                        
                        // Simple progress update
                        if extracted_count % 20 == 0 {
                            println!("Extracted {} portraits so far...", extracted_count);
                        }
                    }
                }
                Err(e) => {
                    println!("Warning: Failed to convert portrait {} to image: {}", pokemon_id, e);
                }
            }
        }
    }

    println!("Successfully extracted {} portraits", extracted_count);
    Ok(())
}
