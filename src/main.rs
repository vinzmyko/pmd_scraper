mod filesystem;
mod pokemon_portrait_extractor;
mod pokemon_sprite_extractor;
mod rom;

mod containers;
mod data;
mod formats;
mod graphics;

use std::{fs, path::PathBuf};

use {
    pokemon_portrait_extractor::PortraitExtractor, pokemon_sprite_extractor::PokemonExtractor,
    rom::Rom,
};

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

    match Rom::new(rom_path) {
        Ok(rom) => {
            println!("Sucessfully parsed ROM, no corruption detected");
            println!("{}", rom.region);
            let sprite_extractor = PokemonExtractor::new(&rom);
            let _ = sprite_extractor.extract_monster_data(None, &output_dir_sprites);

            let portrait_extractor = PortraitExtractor::new(&rom);
            let _ = portrait_extractor.extract_portrait_atlases(&output_dir_portraits);
        }
        Err(e) => {
            eprintln!("Failed to read ROM file, possibly corrupted: {}", e);
        }
    }
}
