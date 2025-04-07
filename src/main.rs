mod containers;
mod data;
mod filesystem;
mod formats;
mod graphics;
mod pokemon_sprite_extractor;
mod rom;

use crate::formats::portrait::KaoFile;

use filesystem::{FileAllocationTable, FileNameTable};
use formats::portrait::{create_portrait_atlas, AtlasType};
use pokemon_sprite_extractor::PokemonExtractor;
use rom::read_header;
use std::fs;
use std::path::PathBuf;

use std::cmp::min;

use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    // Set up paths
    let rom_path = PathBuf::from("../../ROMs/pmd_eos_us.nds");
    let output_dir = PathBuf::from("./output/MONSTER");

    // Create output directory if it doesn't exist
    if !output_dir.exists() {
        if let Err(e) = fs::create_dir_all(&output_dir) {
            eprintln!("Failed to create output directory: {}", e);
            return;
        }
    }

    println!("ROM Path: {:?}", &rom_path);
    println!("Output Dir: {:?}", output_dir);

    let pokemon_ids = vec![1, 4, 6];

    // Create extractor instance
    match PokemonExtractor::new(rom_path) {
        Ok(extractor) => {
            if let Err(e) = extractor.extract_monster_data(Some(&pokemon_ids), &output_dir) {
                eprintln!("Error in focused test: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Failed to open ROM file: {}", e);
        }
    }

    println!("Processing complete!");
}

//let pokedex_atlas_path = output_dir.join("pokedex_atlas.png");
//let expressions_atlas_path = output_dir.join("expressions_atlas.png");

//match create_portrait_atlas(&kao_file, AtlasType::Pokedex, &pokedex_atlas_path) {
//    Ok(atlas) => {
//        println!("Successully created Pokedex atlas!");

//        if let Err(e) = atlas.save(&pokedex_atlas_path) {
//            eprintln!("Failed to save Pokedex atlas: {}", e);
//        } else {
//            println!("Saved Pokedex atlas to {:?}", pokedex_atlas_path);
//        }
//    },
//    Err(e) => {
//        eprintln!("Error creating Pokedex atlas: {}", e);
//    }
//}
//
//match create_portrait_atlas(&kao_file, AtlasType::Expressions, &expressions_atlas_path) {
//    Ok(atlas) => {
//        println!("Successully created Expressions atlas!");

//        if let Err(e) = atlas.save(&expressions_atlas_path) {
//            eprintln!("Failed to save Pokedex atlas: {}", e);
//        } else {
//            println!("Saved Pokedex atlas to {:?}", expressions_atlas_path);
//        }
//    },
//    Err(e) => {
//        eprintln!("Error creating Pokedex atlas: {}", e);
//    }
//}
