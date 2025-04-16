use serde_json;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::data::animation_info::{
    AnimData, EffectAnimationInfo as GeneralAnim, ItemAnimationInfo as ItemAnim,
    MoveAnimationInfo as MoveAnim, SpecialMoveAnimationInfo as SpecMoveAnim,
    TrapAnimationInfo as TrapAnim,
};
use crate::rom::Rom;

pub struct AnimationInfoExtractor<'a> {
    rom: &'a mut Rom,
}

impl<'a> AnimationInfoExtractor<'a> {
    pub fn new(rom: &'a mut Rom) -> Self {
        AnimationInfoExtractor { rom }
    }

    /// Extracts and saves animation data for a specific move and Pokémon
    /// This is the original implementation
    pub fn extract_move_animation(
        &mut self,
        output_dir: &Path,
        move_id: usize,
        pokemon_id: Option<u16>,
    ) -> Result<PathBuf, String> {
        println!(
            "Extractor: ROM HashMap address: {:p}",
            &self.rom.loaded_overlays as *const _
        );
        println!(
            "Extractor: Current overlays before loading: {:?}",
            self.rom.loaded_overlays.keys().collect::<Vec<_>>()
        );

        // Load overlay 10
        self.rom
            .load_arm9_overlays(&[10])
            .map_err(|e| format!("Failed to load overlay 10: {}", e))?;

        println!(
            "Extractor: Overlays immediately after loading: {:?}",
            self.rom.loaded_overlays.keys().collect::<Vec<_>>()
        );

        // Check for immediate state loss
        if !self.rom.loaded_overlays.contains_key(&10) {
            return Err("CRITICAL: Overlay 10 lost immediately after loading!".to_string());
        }

        // Extract animation data
        let anim_data = self.rom.extract_animation_data()?;
        println!("Extracted animation data");

        // Get transformed move data
        let move_map = anim_data.transform_move_data();

        // Verify move ID is valid
        if !move_map.contains_key(&move_id) {
            return Err(format!(
                "Move ID {} is out of range (0-{})",
                move_id,
                move_map.len() - 1
            ));
        }

        // Create output directory
        let move_dir = if let Some(pkmn_id) = pokemon_id {
            output_dir.join(format!("move_{}_pokemon_{}", move_id, pkmn_id))
        } else {
            output_dir.join(format!("move_{}", move_id))
        };
        fs::create_dir_all(&move_dir)
            .map_err(|e| format!("Failed to create output directory: {}", e))?;

        // Get move animation data
        let move_anim = &move_map[&move_id];

        // Save move info
        self.save_move_info(&move_dir, move_id, move_anim, pokemon_id)
            .map_err(|e| format!("Failed to save move info: {}", e))?;

        // Check for special animation for this Pokémon
        if let Some(pkmn_id) = pokemon_id {
            // Look for special animation for this Pokémon in the embedded list
            if let Some(special_anim) = move_anim
                .special_animations
                .iter()
                .find(|spec| spec.pokemon_id == pkmn_id)
            {
                // Save special animation info
                self.save_special_anim_info(&move_dir, special_anim)
                    .map_err(|e| format!("Failed to save special animation info: {}", e))?;
            }
        }

        // Extract effect animations
        let effect_ids = [
            move_anim.effect_id_1,
            move_anim.effect_id_2,
            move_anim.effect_id_3,
            move_anim.effect_id_4,
        ];

        // Use general_table from anim_data for effect animations
        for (i, &effect_id) in effect_ids.iter().enumerate() {
            if effect_id == 0 {
                continue; // Skip empty animations
            }

            if effect_id as usize >= anim_data.general_table.len() {
                println!("Warning: Effect ID {} is out of range", effect_id);
                continue;
            }

            let effect = &anim_data.general_table[effect_id as usize];
            self.save_effect_info(&move_dir, i + 1, effect_id, effect)
                .map_err(|e| format!("Failed to save effect info: {}", e))?;
        }

        println!("Animation data saved to {}", move_dir.display());
        Ok(move_dir)
    }

    /// Extracts all animation data and saves it to JSON files
    pub fn extract_all_animation_data(&mut self, output_dir: &Path) -> Result<PathBuf, String> {
        println!("Starting extraction of all animation data");

        // Load overlay 10
        self.rom
            .load_arm9_overlays(&[10])
            .map_err(|e| format!("Failed to load overlay 10: {}", e))?;

        // Check for immediate state loss
        if !self.rom.loaded_overlays.contains_key(&10) {
            return Err("CRITICAL: Overlay 10 lost immediately after loading!".to_string());
        }

        // Extract animation data
        let anim_data = self.rom.extract_animation_data()?;
        println!("Extracted all animation data tables");

        // Create output directory for JSON files
        let json_dir = output_dir.join("animation_data");
        fs::create_dir_all(&json_dir)
            .map_err(|e| format!("Failed to create JSON output directory: {}", e))?;

        // Serialize and save all animation data tables to JSON
        self.save_trap_animations_json(&json_dir, &anim_data.trap_table)?;
        self.save_item_animations_json(&json_dir, &anim_data.item_table)?;
        self.save_move_animations_json(&json_dir, &anim_data)?; // Updated to pass anim_data
        self.save_general_animations_json(&json_dir, &anim_data.general_table)?;
        // Removed special_move_animations_json as it's now embedded in moves.json

        // Also save a summary file with counts
        self.save_animation_summary(&json_dir, &anim_data)?;

        println!("All animation data saved to {}", json_dir.display());
        Ok(json_dir)
    }

    /// Saves trap animation data to JSON
    fn save_trap_animations_json(&self, dir: &Path, trap_table: &[TrapAnim]) -> Result<(), String> {
        let file_path = dir.join("traps.json");
        let file =
            File::create(&file_path).map_err(|e| format!("Failed to create traps.json: {}", e))?;

        // Create a JSON array of trap animations
        serde_json::to_writer_pretty(file, &trap_table)
            .map_err(|e| format!("Failed to serialize trap animations: {}", e))?;

        println!(
            "Trap animations saved to {} ({} entries)",
            file_path.display(),
            trap_table.len()
        );
        Ok(())
    }

    /// Saves item animation data to JSON
    fn save_item_animations_json(&self, dir: &Path, item_table: &[ItemAnim]) -> Result<(), String> {
        let file_path = dir.join("items.json");
        let file =
            File::create(&file_path).map_err(|e| format!("Failed to create items.json: {}", e))?;

        // Create a JSON array of item animations
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
    /// Updated to use the transformed data with embedded special animations
    fn save_move_animations_json(&self, dir: &Path, anim_data: &AnimData) -> Result<(), String> {
        let file_path = dir.join("moves.json");
        let file =
            File::create(&file_path).map_err(|e| format!("Failed to create moves.json: {}", e))?;

        // Transform the raw move data to the final format with embedded special animations
        let move_map = anim_data.transform_move_data();

        // Create a JSON object mapping move_id to move animation data
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

    /// Saves general/effect animation data to JSON
    fn save_general_animations_json(
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

    /// Saves a summary of all animation data
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

    // Original helper methods below, updated to use the new data structures

    fn save_move_info(
        &self,
        dir: &Path,
        move_id: usize,
        move_anim: &MoveAnim,
        pokemon_id: Option<u16>,
    ) -> std::io::Result<()> {
        let mut file = File::create(dir.join("move_info.txt"))?;

        writeln!(file, "=== MOVE ID {} ===", move_id)?;
        writeln!(file, "Main Animation: {}", move_anim.animation)?;
        writeln!(
            file,
            "Effect Animations: {}, {}, {}, {}",
            move_anim.effect_id_1,
            move_anim.effect_id_2,
            move_anim.effect_id_3,
            move_anim.effect_id_4
        )?;
        writeln!(file, "Direction: {}", move_anim.dir)?;
        writeln!(
            file,
            "Flags: {}, {}, {}, {}",
            move_anim.flag1 as u8,
            move_anim.flag2 as u8,
            move_anim.flag3 as u8,
            move_anim.flag4 as u8
        )?;
        writeln!(file, "Speed: {}", move_anim.speed)?;
        writeln!(file, "Position: {}", move_anim.point)?;
        writeln!(file, "Sound Effect: {}", move_anim.sfx_id)?;

        if !move_anim.special_animations.is_empty() {
            writeln!(
                file,
                "\nSpecial Animations: {} entries",
                move_anim.special_animations.len()
            )?;

            if let Some(pkmn_id) = pokemon_id {
                writeln!(
                    file,
                    "Checking for special animation for Pokémon ID {}",
                    pkmn_id
                )?;
            }
        }

        Ok(())
    }

    fn save_special_anim_info(
        &self,
        dir: &Path,
        special_anim: &SpecMoveAnim,
    ) -> std::io::Result<()> {
        let mut file = File::create(dir.join("special_animation_info.txt"))?;

        writeln!(
            file,
            "=== SPECIAL ANIMATION FOR POKEMON ID {} ===",
            special_anim.pokemon_id
        )?;
        writeln!(file, "Animation: {}", special_anim.user_animation_index)?;
        writeln!(file, "Position: {}", special_anim.point)?;
        writeln!(file, "Sound Effect: {}", special_anim.sfx_id)?;

        Ok(())
    }

    fn save_effect_info(
        &self,
        dir: &Path,
        index: usize,
        effect_id: u16,
        effect: &GeneralAnim,
    ) -> std::io::Result<()> {
        let effect_dir = dir.join(format!("effect_{}", index));
        fs::create_dir_all(&effect_dir)?;

        let mut file = File::create(effect_dir.join("effect_info.txt"))?;

        writeln!(file, "=== EFFECT {} (ID: {}) ===", index, effect_id)?;
        writeln!(
            file,
            "Animation Type: {} ({})",
            effect.anim_type, effect.anim_type as u32
        )?;
        writeln!(file, "File Index: {}", effect.file_index)?;
        writeln!(file, "Animation Index: {}", effect.animation_index)?;
        writeln!(file, "Sound Effect ID: {}", effect.sfx_id)?;
        writeln!(file, "Position: {}", effect.point)?;
        writeln!(file, "Looping: {}", effect.loop_flag)?;

        Ok(())
    }
}
