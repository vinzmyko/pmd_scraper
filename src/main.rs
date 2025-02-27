mod filesystem;
mod formats;
mod rom;

use crate::formats::portrait::KaoFile;

use filesystem::{FileAllocationTable, FileNameTable};
use rom::read_header;
use std::fs;
use std::path::PathBuf;

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

fn extract_portraits(rom_path: &PathBuf, output_dir: &PathBuf) -> Result<(), String> {
    // Read ROM data once - this is a key optimization we want to keep
    let rom_data = fs::read(rom_path).map_err(|e| format!("Failed to read ROM file: {}", e))?;

    let header = read_header(rom_path);

    // Maybe change this to take a function to take a ROM header instead?
    let fat = FileAllocationTable::read_from_rom(&rom_data, header.fat_offset, header.fat_size)
        .map_err(|e| format!("Failed to read FAT: {}", e))?;

    let fnt = FileNameTable::read_from_rom(&rom_data, header.fnt_offset)
        .map_err(|e| format!("Failed to read FNT: {}", e))?;

    let kao_id = fnt
        .get_file_id("FONT/kaomado.kao")
        .ok_or("Could not find kaomado.kao")?;

    let kao_data = fat
        .get_file_data(kao_id as usize, &rom_data)
        .ok_or("Could not read KAO file data")?;

    println!("Found KAO file: FONT/kaomado.kao (ID: {})", kao_id);
    println!("KAO file size: {} bytes", kao_data.len());

    let kao_file = KaoFile::from_bytes(kao_data.to_vec())?;

    // Create output directory if needed
    fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    // For your use case: extract all portraits that exist
    println!("Extracting portraits to build sprite atlases...");
    let mut extracted_count = 0;

    for pokemon_id in 0..=600 {
        if let Ok(Some(portrait)) = kao_file.get_portrait(pokemon_id, 0) {
            let output_path = output_dir.join(format!("pokemon_{:03}.png", pokemon_id));

            match portrait.to_rgba_image() {
                Ok(image) => {
                    if let Err(e) = image.save(&output_path) {
                        println!("Warning: Failed to save portrait {}: {}", pokemon_id, e);
                    } else {
                        extracted_count += 1;

                        if extracted_count % 20 == 0 {
                            println!("Extracted {} portraits so far...", extracted_count);
                        }
                    }
                }
                Err(e) => {
                    println!(
                        "Warning: Failed to convert portrait {} to image: {}",
                        pokemon_id, e
                    );
                }
            }
        }
    }

    println!("Successfully extracted {} portraits", extracted_count);
    Ok(())
}
