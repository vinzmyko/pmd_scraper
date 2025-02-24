mod filesystem;
mod formats;
mod rom;

//use crate::formats::containers::{SUBENTRIES, SUBENTRY_LEN, FIRST_TOC_OFFSET};
use crate::formats::portrait::KaoFile;

use filesystem::{FileAllocationTable, FileNameTable};
use rom::{read_header, Rom};
use std::fs;
use std::path::PathBuf;

fn main() {
    let rom_eu = Rom::new("../../ROMs/pmd_eos_us.nds");
    let header = read_header(&rom_eu.path);
    println!("ROM Header: {:#?}", header);

    let rom_data = std::fs::read(&rom_eu.path).expect("Failed to read ROM file");

    let fat =
        match FileAllocationTable::read_from_rom(&rom_data, header.fat_offset, header.fat_size) {
            Ok(fat) => fat,
            Err(e) => {
                println!("Error reading FAT: {}", e);
                return;
            }
        };

    println!("FAT contains {} entries", fat.entries.len());

    let fnt = match FileNameTable::read_from_rom(&rom_data, header.fnt_offset) {
        Ok(fnt) => fnt,
        Err(e) => {
            println!("Error reading FNT: {}", e);
            return;
        }
    };

    println!("\nFNT contains:");
    println!("- {} directories", fnt.directories.len());
    println!("- {} files\n", fnt.file_names.len());

    let output_dir = PathBuf::from("./output/FONT");

    // Create output directory if it doesn't exist
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).expect("Failed to create output directory");
    }

    println!("ROM Path: {:?}", &rom_eu.path);
    println!("Output Dir: {:?}", output_dir);

    match extract_portraits(&rom_eu.path, &output_dir) {
        Ok(_) => println!("Successfully extracted portraits!"),
        Err(e) => println!("Error extracting portraits: {}", e),
    }
}

fn extract_portraits(rom_path: &PathBuf, output_dir: &PathBuf) -> Result<(), String> {
    // Read ROM data
    let rom_data = fs::read(rom_path).map_err(|e| format!("Failed to read ROM file: {}", e))?;

    // Read header
    let header = read_header(rom_path);

    // Read FAT
    let fat = FileAllocationTable::read_from_rom(&rom_data, header.fat_offset, header.fat_size)
        .map_err(|e| format!("Failed to read FAT: {}", e))?;

    // Read FNT
    let fnt = FileNameTable::read_from_rom(&rom_data, header.fnt_offset)
        .map_err(|e| format!("Failed to read FNT: {}", e))?;

    // Find the KAO file
    let kao_id = fnt.get_file_id("FONT/kaomado.kao").ok_or("Could not find kaomado.kao")?;
    
    // Get KAO file data
    let kao_data = fat.get_file_data(kao_id as usize, &rom_data)
        .ok_or("Could not read KAO file data")?;

    println!("Found KAO file: FONT/kaomado.kao (ID: {})", kao_id);
    println!("KAO file size: {} bytes", kao_data.len());

    // Create KaoFile from the data
    let kao_file = KaoFile::from_bytes(kao_data.to_vec())?;

    // Create output directory if it doesn't exist
    fs::create_dir_all(output_dir).map_err(|e| format!("Failed to create output directory: {}", e))?;

    for pokemon_id in 0..= 100 {
        // Get portrait at subindex 0 (normal portrait)
        if let Ok(Some(portrait)) = kao_file.get_portrait(pokemon_id, 0) {
            let output_path = output_dir.join(format!("pokemon_{:03}.png", pokemon_id));
            
            match portrait.to_rgba_image() {
                Ok(image) => {
                    image.save(&output_path)
                        .map_err(|e| format!("Failed to save portrait {}: {}", pokemon_id, e))?;
                    println!("Saved portrait {} to {:?}", pokemon_id, output_path);
                }
                Err(e) => println!("Failed to convert portrait {} to image: {}", pokemon_id, e),
            }
        }
    }

    Ok(())
}
