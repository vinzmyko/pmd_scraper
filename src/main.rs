mod rom;
mod filesystem;
mod pokemon_portrait_extractor;
mod pokemon_sprite_extractor;

mod containers;
mod graphics;
mod data;
mod formats;

use std::{fs, path::PathBuf};

use pokemon_portrait_extractor::PortraitExtractor;
use pokemon_sprite_extractor::PokemonExtractor;

fn main() {
    let rom_path = PathBuf::from("../../ROMs/pmd_eos_us.nds");
    let output_dir_sprites = PathBuf::from("./output/MONSTER");
    let output_dir_portraits = PathBuf::from("./output/FONT");

    // Create output directory if it doesn't exist
    if !output_dir_sprites.exists() {
        if let Err(e) = fs::create_dir_all(&output_dir_sprites) {
            eprintln!("Failed to create output directory: {}", e);
            return;
        }
    }

    if !output_dir_portraits.exists() {
        if let Err(e) = fs::create_dir_all(&output_dir_portraits) {
            eprintln!("Failed to create output directory: {}", e);
            return;
        }
    }

    println!("ROM Path: {:?}", &rom_path);

    // For debugging
    //let id_range = 199 ..= 250;
    //let pokemon_ids: Vec<usize> = id_range.collect();
    //let pokemon_ids = vec![1];

    match PokemonExtractor::new(&rom_path) {
        Ok(extractor) => {
            println!("Sprites Output Dir: {:?}", output_dir_sprites);
            if let Err(e) = extractor.extract_monster_data(None, &output_dir_sprites)
            {
                eprintln!("Error in focused test: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Failed to open ROM file: {}", e);
        }
    }

    println!("Sprites Output Dir: {:?}", output_dir_sprites);

    match PortraitExtractor::new(&rom_path) {
        Ok(extractor) => {
            println!("Portraits Output Dir: {:?}", output_dir_sprites);
            if let Err(e) = extractor.extract_portrait_atlases(&output_dir_portraits) {
                eprintln!("Error extracting portraits: {}", e);
            }
        },
        Err(e) => {
            eprintln!("Failed to create portrait extractor: {}", e);
        }
    }

    println!("Processing complete!");
}
