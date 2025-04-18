mod animation_info_extractor;
mod arm9;
mod binary_utils;
mod effect_sprite_extractor;
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
    animation_info_extractor::AnimationInfoExtractor,
    pokemon_portrait_extractor::PortraitExtractor,
    pokemon_sprite_extractor::PokemonSpriteExtractor, rom::Rom,
};

fn main() {
    let rom_path = PathBuf::from("../../ROMs/pmd_eos_us.nds");
    let output_dir_sprites = PathBuf::from("./output/MONSTER");
    let output_dir_portraits = PathBuf::from("./output/FONT");
    let output_dir_animations = PathBuf::from("./output/DATA");

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

    if !output_dir_animations.exists() {
        if let Err(e) = fs::create_dir_all(&output_dir_animations) {
            eprintln!("Failed to create output directory: {}", e);
            return;
        }
    }

    match Rom::new(rom_path) {
        Ok(mut rom) => {
            println!("Successfully parsed ROM, no corruption detected");

            let mut animation_info_extractor = AnimationInfoExtractor::new(&mut rom);

            println!("Extracting all animation data...");
            let anim_data_info = animation_info_extractor.parse_and_transform_animation_data();
            let _ = animation_info_extractor
                .save_animation_info_json(&anim_data_info, &output_dir_animations);

            //let sprite_extractor = PokemonExtractor::new(&rom);
            //let _ = sprite_extractor.extract_monster_data(None, &output_dir_sprites);

            //let portrait_extractor = PortraitExtractor::new(&rom);
            //let _ = portrait_extractor.extract_portrait_atlases(&output_dir_portraits);
        }
        Err(e) => {
            eprintln!("Failed to read ROM file, possibly corrupted: {}", e);
        }
    }
}
