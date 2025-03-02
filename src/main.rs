mod filesystem;
mod formats;
mod rom;

use crate::formats::portrait::KaoFile;

use filesystem::{FileAllocationTable, FileNameTable};
use rom::read_header;
use formats::portrait::{create_portrait_atlas, AtlasType};
use std::fs;
use std::path::PathBuf;

use std::sync::{Arc, Mutex};
use std::thread;

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

    let rom_data = fs::read(&rom_path).unwrap();

    let header = read_header(&rom_path);

    // Maybe change this to take a function to take a ROM header instead?
    let fat =
        FileAllocationTable::read_from_rom(&rom_data, header.fat_offset, header.fat_size).unwrap();

    let fnt = FileNameTable::read_from_rom(&rom_data, header.fnt_offset).unwrap();

    let kao_id = fnt
        .get_file_id("FONT/kaomado.kao")
        .ok_or("Could not find kaomado.kao")
        .unwrap();

    let kao_data = fat
        .get_file_data(kao_id as usize, &rom_data)
        .ok_or("Could not read KAO file data")
        .unwrap();

    println!("Found KAO file: FONT/kaomado.kao (ID: {})", kao_id);
    println!("KAO file size: {} bytes", kao_data.len());

    let kao_file = KaoFile::from_bytes(kao_data.to_vec()).unwrap();

    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("Failed to create output directory: {}", e))
        .unwrap();

    let pokedex_atlas_path = output_dir.join("pokedex_atlas.png");
    let expressions_atlas_path = output_dir.join("expressions_atlas.png");

    match create_portrait_atlas(&kao_file, AtlasType::Pokedex, &pokedex_atlas_path) {
        Ok(atlas) => {
            println!("Successully created Pokedex atlas!");

            if let Err(e) = atlas.save(&pokedex_atlas_path) {
                eprintln!("Failed to save Pokedex atlas: {}", e);
            } else {
                println!("Saved Pokedex atlas to {:?}", pokedex_atlas_path);
            }
        },
        Err(e) => {
            eprintln!("Error creating Pokedex atlas: {}", e);
        }
    }
    
    match create_portrait_atlas(&kao_file, AtlasType::Expressions, &expressions_atlas_path) {
        Ok(atlas) => {
            println!("Successully created Expressions atlas!");

            if let Err(e) = atlas.save(&expressions_atlas_path) {
                eprintln!("Failed to save Pokedex atlas: {}", e);
            } else {
                println!("Saved Pokedex atlas to {:?}", expressions_atlas_path);
            }
        },
        Err(e) => {
            eprintln!("Error creating Pokedex atlas: {}", e);
        }
    }
}
