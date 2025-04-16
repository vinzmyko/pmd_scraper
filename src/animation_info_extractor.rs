use std::{
    collections::HashMap,
    fs::{self, File},
    path::{Path, PathBuf},
};

use crate::{
    data::animation_info::{
        AnimData, EffectAnimationInfo as GeneralAnim, ItemAnimationInfo as ItemAnim,
        MoveAnimationInfo as MoveAnim, TrapAnimationInfo as TrapAnim,
    },
    rom::Rom,
};

use serde_json;

pub struct AnimationInfoExtractor<'a> {
    rom: &'a mut Rom,
}

impl<'a> AnimationInfoExtractor<'a> {
    pub fn new(rom: &'a mut Rom) -> Self {
        AnimationInfoExtractor { rom }
    }

    /// Extracts all animation data and saves it to JSON files
    pub fn extract_all_animation_data(&mut self, output_dir: &Path) -> Result<PathBuf, String> {
        println!("Starting extraction of all animation data");

        self.rom
            .load_arm9_overlays(&[10])
            .map_err(|e| format!("Failed to load overlay 10: {}", e))?;

        let anim_data = self.rom.extract_animation_data()?;
        println!("Extracted all animation data tables");

        let json_dir = output_dir.join("animation_data");
        fs::create_dir_all(&json_dir)
            .map_err(|e| format!("Failed to create JSON output directory: {}", e))?;

        self.save_trap_animations_json(&json_dir, &anim_data.trap_table)?;
        self.save_item_animations_json(&json_dir, &anim_data.item_table)?;
        self.save_move_animations_json(&json_dir, &anim_data)?;
        self.save_effect_animations_json(&json_dir, &anim_data.general_table)?;

        self.save_animation_summary(&json_dir, &anim_data)?;

        println!("All animation data saved to {}", json_dir.display());
        Ok(json_dir)
    }

    /// Saves trap animation data to JSON
    fn save_trap_animations_json(&self, dir: &Path, trap_table: &[TrapAnim]) -> Result<(), String> {
        let file_path = dir.join("traps.json");
        let file =
            File::create(&file_path).map_err(|e| format!("Failed to create traps.json: {}", e))?;

        serde_json::to_writer_pretty(file, &trap_table)
            .map_err(|e| format!("Failed to serialize trap animations: {}", e))?;

        println!(
            "Trap animations saved to {} ({} entries)",
            file_path.display(),
            trap_table.len()
        );
        Ok(())
    }

    fn save_item_animations_json(&self, dir: &Path, item_table: &[ItemAnim]) -> Result<(), String> {
        let file_path = dir.join("items.json");
        let file =
            File::create(&file_path).map_err(|e| format!("Failed to create items.json: {}", e))?;

        serde_json::to_writer_pretty(file, &item_table)
            .map_err(|e| format!("Failed to serialize item animations: {}", e))?;

        println!(
            "Item animations saved to {} ({} entries)",
            file_path.display(),
            item_table.len()
        );
        Ok(())
    }

    /// Saves move animation data to JSON as an object mapping move IDs to animation data
    fn save_move_animations_json(&self, dir: &Path, anim_data: &AnimData) -> Result<(), String> {
        let file_path = dir.join("moves.json");
        let file =
            File::create(&file_path).map_err(|e| format!("Failed to create moves.json: {}", e))?;

        // Transform the raw move data to the final format with embedded special animations
        let move_map = anim_data.transform_move_data();

        let move_map_str: HashMap<String, &MoveAnim> = move_map
            .iter()
            .map(|(idx, anim)| (idx.to_string(), anim))
            .collect();

        serde_json::to_writer_pretty(file, &move_map_str)
            .map_err(|e| format!("Failed to serialize move animations: {}", e))?;

        println!(
            "Move animations saved to {} ({} entries)",
            file_path.display(),
            move_map.len()
        );
        Ok(())
    }

    fn save_effect_animations_json(
        &self,
        dir: &Path,
        general_table: &[GeneralAnim],
    ) -> Result<(), String> {
        let file_path = dir.join("effects.json");
        let file = File::create(&file_path)
            .map_err(|e| format!("Failed to create effects.json: {}", e))?;

        // Create a JSON object mapping effect_id to general animation data
        let effect_map: HashMap<String, &GeneralAnim> = general_table
            .iter()
            .enumerate()
            .map(|(idx, anim)| (idx.to_string(), anim))
            .collect();

        serde_json::to_writer_pretty(file, &effect_map)
            .map_err(|e| format!("Failed to serialize effect animations: {}", e))?;

        println!(
            "Effect animations saved to {} ({} entries)",
            file_path.display(),
            general_table.len()
        );
        Ok(())
    }

    fn save_animation_summary(&self, dir: &Path, anim_data: &AnimData) -> Result<(), String> {
        let file_path = dir.join("summary.json");
        let file = File::create(&file_path)
            .map_err(|e| format!("Failed to create summary.json: {}", e))?;

        let summary = serde_json::json!({
            "trap_table_count": anim_data.trap_table.len(),
            "item_table_count": anim_data.item_table.len(),
            "move_table_count": anim_data.raw_move_table.len(),
            "general_table_count": anim_data.general_table.len(),
            "special_move_table_count": anim_data.special_move_table.len(),
            "game_id": self.rom.id_code,
            "region": match self.rom.id_code.chars().last() {
                Some('E') => "NA",
                Some('P') => "EU",
                Some('J') => "JP",
                _ => "Unknown"
            }
        });

        serde_json::to_writer_pretty(file, &summary)
            .map_err(|e| format!("Failed to serialize animation summary: {}", e))?;

        println!("Animation summary saved to {}", file_path.display());
        Ok(())
    }
}
