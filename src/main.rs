mod animation_info_extractor;
mod arm9;
mod binary_utils;
mod effect_sprite_extractor;
mod filesystem;
mod move_effects_index;
mod pokemon_portrait_extractor;
mod pokemon_sprite_extractor;
mod rom;

mod containers;
mod data;
mod formats;
mod graphics;

use std::{collections::HashMap, fs, path::PathBuf};

use {
    animation_info_extractor::AnimationInfoExtractor, effect_sprite_extractor::EffectAssetPipeline,
    pokemon_portrait_extractor::PortraitExtractor,
    pokemon_sprite_extractor::PokemonSpriteExtractor, rom::Rom,
};

fn main() {
    let rom_path = PathBuf::from("../../ROMs/pmd_eos_us.nds");

    let output_dir_sprites = PathBuf::from("./output/MONSTER");
    let output_dir_portraits = PathBuf::from("./output/FONT");
    let output_dir_jsons = PathBuf::from("./output/DATA"); // To store original JSON dumps
    let output_dir_pipeline = PathBuf::from("./output"); // Base directory for the pipeline output

    for dir in [
        &output_dir_sprites,
        &output_dir_portraits,
        &output_dir_jsons,
        &output_dir_pipeline,
    ] {
        if !dir.exists() {
            fs::create_dir_all(dir).expect("Failed to create output directory");
        }
    }

    match Rom::new(rom_path) {
        Ok(mut rom) => {
            println!("Successfully parsed ROM, no corruption detected");

            let mut animation_info_extractor = AnimationInfoExtractor::new(&mut rom);
            println!("Extracting all animation data...");
            let anim_data_info = animation_info_extractor.parse_and_transform_animation_data();
            let _ = animation_info_extractor
                .save_animation_info_json(&anim_data_info, &output_dir_jsons);

            let effects_map: HashMap<u16, _> = anim_data_info
                .effect_table
                .clone()
                .into_iter()
                .enumerate()
                .map(|(idx, info)| (idx as u16, info))
                .collect();

            let moves_map = anim_data_info.transform_move_data();

            let mut effect_pipeline = EffectAssetPipeline::new(&rom);
            let _ = effect_pipeline.run(&effects_map, &moves_map, &output_dir_pipeline);

            let sprite_extractor = PokemonSpriteExtractor::new(&rom);
            let _ = sprite_extractor.extract_monster_data(None, &output_dir_sprites);

            let portrait_extractor = PortraitExtractor::new(&rom);
            let _ = portrait_extractor.extract_portrait_atlases(&output_dir_portraits);
        }
        Err(e) => {
            eprintln!("Failed to read ROM file, possibly corrupted: {}", e);
        }
    }
}
