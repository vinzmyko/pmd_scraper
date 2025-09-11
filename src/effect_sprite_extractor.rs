use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self},
    path::Path,
};

use crate::{
    containers::{
        binpack::BinPack,
        compression::pkdpx::PkdpxContainer,
        sir0::{self},
        ContainerHandler,
    },
    data::animation_info::{AnimType, EffectAnimationInfo, MoveAnimationInfo},
    graphics::{
        wan::{
            model::WanFile,
            parser::{parse_wan_from_sir0_content, parse_wan_palette_only},
            renderer, AnimationStructure, PaletteList,
        },
        WanType,
    },
    move_effects_index::{
        AnimationDetails, AnimationSequence, EffectDefinition, MoveData, MoveEffectTrigger,
        MoveEffectsIndex, ReuseEffect, ScreenEffect, SpriteEffect,
    },
    rom::Rom,
};

/// Handles the entire pipeline of extracting effect sprites and compiling the move/effect index
pub struct EffectAssetPipeline<'a> {
    rom: &'a Rom,
    wan_cache: HashMap<usize, WanFile>,
    effect_bin: Option<BinPack>,
    base_palette: Option<PaletteList>,
}

impl<'a> EffectAssetPipeline<'a> {
    pub fn new(rom: &'a Rom) -> Self {
        EffectAssetPipeline {
            rom,
            wan_cache: HashMap::new(),
            effect_bin: None,
            base_palette: None,
        }
    }

    /// Renders sprites, saves them, and generates a final `asset_index.json`
    pub fn run(
        &mut self,
        effects_map: &HashMap<u16, EffectAnimationInfo>,
        moves_map: &HashMap<usize, MoveAnimationInfo>,
        output_dir: &Path,
    ) -> io::Result<()> {
        println!("\n--- Starting Effect Asset Pipeline ---");

        self.load_bin_containers()?;

        let sprites_dir = output_dir.join("EFFECT");
        fs::create_dir_all(&sprites_dir)?;

        let mut index = MoveEffectsIndex::new();
        let mut effects_processed = 0;
        let mut effects_skipped = 0;
        let mut errors = 0;

        let mut sorted_effect_ids: Vec<_> = effects_map.keys().collect();
        sorted_effect_ids.sort();

        for effect_id in sorted_effect_ids {
            let effect_info = &effects_map[effect_id];
            let anim_type = effect_info.anim_type;

            println!(
                "Processing Effect ID: {} (Type: {:?})",
                effect_id, anim_type
            );

            let effect_entry = match anim_type {
                AnimType::WanOther => {
                    match self.process_sprite_effect(*effect_id, effect_info, &sprites_dir) {
                        Ok(Some(entry)) => {
                            effects_processed += 1;
                            Some(entry)
                        }
                        Ok(None) => {
                            effects_skipped += 1; // Empty animations
                            None
                        }
                        Err(e) => {
                            eprintln!(" -> ERROR processing effect {}: {}", effect_id, e);
                            errors += 1;
                            None
                        }
                    }
                }
                AnimType::WanFile0 => {
                    effects_skipped += 1;
                    Some(EffectDefinition::Reuse(ReuseEffect {
                        target: "Attacker".to_string(),
                        animation_index: effect_info.animation_index,
                    }))
                }
                AnimType::Screen => {
                    effects_skipped += 1;
                    Some(EffectDefinition::Screen(ScreenEffect {
                        effect_name: format!("ScreenEffect_{}", effect_id),
                    }))
                }
                _ => {
                    println!(" -> Skipping: Unsupported type");
                    effects_skipped += 1;
                    None
                }
            };

            if let Some(entry) = effect_entry {
                index.effects.insert(effect_id.to_string(), entry);
            }
        }

        println!("Populating moves data...");
        self.populate_moves_data(&mut index, moves_map);

        // Write the complete index to disk
        self.save_index(&index, output_dir)?;

        println!("\n---------------------------------");
        println!("Effect Asset Pipeline Complete!");
        println!("  Sprites Processed: {}", effects_processed);
        println!("  Effects Skipped (by design): {}", effects_skipped);
        println!("  Errors: {}", errors);
        println!("---------------------------------");

        Ok(())
    }

    /// Renders, saves, and builds the definition for a 'WanOther' type effect
    fn process_sprite_effect(
        &mut self,
        effect_id: u16,
        effect_info: &EffectAnimationInfo,
        sprites_dir: &Path,
    ) -> io::Result<Option<EffectDefinition>> {
        let file_index = effect_info.file_index as usize;
        // Use the animation_index from the JSON file
        let anim_index = effect_info.animation_index as usize;

        // Cache already scanned effect sprites
        self.ensure_effect_wan_cached(file_index)?;

        let wan_file = self.wan_cache.get(&file_index).unwrap();

        // Render the sprite sheet in memory
        match renderer::render_effect_animation_sheet(wan_file, anim_index) {
            Ok(Some((sprite_sheet, frame_width, frame_height))) => {
                // Save the in memory image buffer to disk
                let sheet_filename = format!("{}.png", effect_id);
                let sheet_path = sprites_dir.join(&sheet_filename);
                self.save_effect_sprite_png(&sprite_sheet, &sheet_path)?;
                println!(
                    " -> SUCCESS: Sprite sheet saved to {}",
                    sheet_path.display()
                );

                let effect_definition = self.build_sprite_effect_definition(
                    wan_file,
                    effect_id,
                    anim_index,
                    frame_width,
                    frame_height,
                );
                Ok(Some(effect_definition))
            }
            Ok(None) => {
                println!(" -> WARNING: Animation is empty or has no visible pixels. Skipping.");
                Ok(None)
            }
            Err(e) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to render sprite sheet: {:?}", e),
            )),
        }
    }

    /// Builds the `SpriteEffect` data structure from a rendered animation
    fn build_sprite_effect_definition(
        &self,
        wan_file: &WanFile,
        effect_id: u16,
        animation_index: usize,
        frame_width: u32,
        frame_height: u32,
    ) -> EffectDefinition {
        let animation_sequence = match &wan_file.animations {
            AnimationStructure::Effect(anims) => anims.get(animation_index),
            AnimationStructure::Character(_) => None,
        };
        let animation_sequence = match animation_sequence {
            Some(anim) => anim,
            None => {
                return EffectDefinition::Sprite(SpriteEffect {
                    sprite_sheet: format!("res://effect_sprites/{}.png", effect_id),
                    frame_width: 1,
                    frame_height: 1,
                    animations: HashMap::new(),
                });
            }
        };

        let frame_details: Vec<[f32; 3]> = animation_sequence
            .frames
            .iter()
            .map(|frame| {
                let duration_sec = (frame.duration as f32 / 60.0 * 10000.0).round() / 10000.0;
                [duration_sec, frame.offset.0 as f32, frame.offset.1 as f32]
            })
            .collect();

        // Check if all frames have the same duration and zero offset
        let is_simple = if frame_details.len() > 1 {
            let first_duration = frame_details[0][0];
            frame_details.iter().all(|frame| {
                (frame[0] - first_duration).abs() < f32::EPSILON
                    && frame[1] == 0.0
                    && frame[2] == 0.0
            })
        } else {
            // A single frame animation is simple if its offset is zero
            matches!(frame_details.first(), Some(f) if f[1] == 0.0 && f[2] == 0.0)
        };

        let animation_details = if is_simple {
            AnimationDetails::Simple {
                frame_count: frame_details.len(),
                duration: frame_details[0][0],
            }
        } else {
            AnimationDetails::Complex {
                frames: frame_details,
            }
        };

        let mut animations = HashMap::new();
        animations.insert(
            "play".to_string(),
            AnimationSequence {
                looping: false, // TODO: This should come from effect_info.loop_flag
                details: animation_details,
            },
        );

        EffectDefinition::Sprite(SpriteEffect {
            sprite_sheet: format!("res://effect_sprites/{}.png", effect_id),
            frame_width,
            frame_height,
            animations,
        })
    }

    /// Populates the `moves` section of the index.
    fn populate_moves_data(
        &self,
        index: &mut MoveEffectsIndex,
        moves_map: &HashMap<usize, MoveAnimationInfo>,
    ) {
        let mut sorted_move_ids: Vec<_> = moves_map.keys().collect();
        sorted_move_ids.sort();

        for move_id in sorted_move_ids {
            let move_info = &moves_map[move_id];
            let mut move_effects = Vec::new();

            let effect_ids = [
                move_info.effect_id_1,
                move_info.effect_id_2,
                move_info.effect_id_3,
                move_info.effect_id_4,
            ];

            for &effect_id in &effect_ids {
                if effect_id > 0 && index.effects.contains_key(&effect_id.to_string()) {
                    move_effects.push(MoveEffectTrigger {
                        id: effect_id.to_string(),
                        trigger: "OnExecute".to_string(),
                    });
                }
            }

            if !move_effects.is_empty() {
                index.moves.insert(
                    move_id.to_string(),
                    MoveData {
                        effects: move_effects,
                    },
                );
            }
        }
    }

    /// Caches a WAN file if it's not already loaded.
    fn ensure_effect_wan_cached(&mut self, effect_index: usize) -> io::Result<()> {
        if self.wan_cache.contains_key(&effect_index) {
            return Ok(());
        }

        let effect_bin = self
            .effect_bin
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "effect.bin not loaded"))?;
        if effect_index >= effect_bin.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Effect index {} out of range for effect.bin", effect_index),
            ));
        }

        let sprite_data = &effect_bin[effect_index];
        let mut wan_file = self.parse_wan_from_data(sprite_data, WanType::Effect, false)?;

        if let Some(base_palette) = &self.base_palette {
            if wan_file.palette_offset > 0 {
                let mut merged_palette = base_palette.clone();
                let effect_own_palette = wan_file.custom_palette.clone();
                let offset = wan_file.palette_offset as usize;

                for (i, effect_row) in effect_own_palette.iter().enumerate() {
                    let target_idx = offset + i;
                    while merged_palette.len() <= target_idx {
                        merged_palette.push(vec![(0, 0, 0, 0); effect_row.len()]);
                    }
                    merged_palette[target_idx] = effect_row.clone();
                }

                wan_file.custom_palette = merged_palette;
                wan_file.palette_offset = 0;
            }
        }

        self.wan_cache.insert(effect_index, wan_file);
        Ok(())
    }

    fn save_index(&self, index: &MoveEffectsIndex, output_dir: &Path) -> io::Result<()> {
        let output_path = output_dir.join("asset_index.json");
        println!("Writing final index to {}...", output_path.display());

        let file = File::create(&output_path)?;
        serde_json::to_writer_pretty(file, index)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(())
    }

    fn save_effect_sprite_png(&self, image: &image::RgbaImage, path: &Path) -> io::Result<()> {
        let temp_path = path.with_extension("temp.png");
        image
            .save(&temp_path)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // No difference between compression setting 6 and 2 size wise just so much faster
        let mut options = oxipng::Options::from_preset(2);
        options.bit_depth_reduction = true;
        options.interlace = None;

        match oxipng::optimize(
            &oxipng::InFile::Path(temp_path.clone()),
            &oxipng::OutFile::Path(Some(path.to_path_buf())),
            &options,
        ) {
            Ok(_) => {
                let _ = fs::remove_file(temp_path);
                Ok(())
            }
            Err(e) => {
                fs::rename(temp_path, path)?;
                eprintln!(
                    "Warning: oxipng optimisation failed for {}: {}. File saved unoptimised.",
                    path.display(),
                    e
                );
                Ok(())
            }
        }
    }

    fn load_bin_containers(&mut self) -> io::Result<()> {
        if self.effect_bin.is_some() {
            return Ok(());
        }

        let effect_bin_id = self
            .rom
            .fnt
            .get_file_id("EFFECT/effect.bin")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "effect.bin not found"))?;
        let effect_bin_data = self
            .rom
            .fat
            .get_file_data(effect_bin_id as usize, &self.rom.data)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Failed to extract effect.bin")
            })?;

        println!("Parsing effect.bin...");
        let effect_bin = BinPack::from_bytes(effect_bin_data)?;
        println!(
            " -> Success. Found {} files in the archive.",
            effect_bin.len()
        );

        let base_palette_index = 292;
        if effect_bin.len() > base_palette_index {
            println!(
                "Loading Base Palette from effect.bin[{}]...",
                base_palette_index
            );
            let base_palette_data = &effect_bin[base_palette_index];
            match self.parse_wan_from_data(base_palette_data, WanType::Effect, true) {
                Ok(base_wan) => {
                    self.base_palette = Some(base_wan.custom_palette);
                    println!(" -> Base Palette loaded successfully.");
                }
                Err(e) => {
                    eprintln!(
                        "Fatal Error: Could not load the base palette: {}. Cannot continue.",
                        e
                    );
                    return Err(e);
                }
            }
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "effect.bin is too small to contain the base palette.",
            ));
        }

        self.effect_bin = Some(effect_bin);
        Ok(())
    }

    fn parse_wan_from_data(
        &self,
        data: &[u8],
        wan_type: WanType,
        palette_only: bool,
    ) -> io::Result<WanFile> {
        let decompressed_data = if data.starts_with(b"PKDPX") {
            match PkdpxContainer::deserialise(data) {
                Ok(pkdpx) => pkdpx.decompress().map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("PKDPX decompress error: {}", e),
                    )
                })?,
                Err(e) => return Err(e),
            }
        } else {
            data.to_vec()
        };

        if !decompressed_data.starts_with(b"SIR0") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Data is not in SIR0 format",
            ));
        }

        let sir0_data = sir0::Sir0::from_bytes(&decompressed_data)?;
        if sir0_data.data_pointer as usize >= sir0_data.content.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "SIR0 data pointer out of bounds",
            ));
        }

        let parse_result = if palette_only {
            parse_wan_palette_only(&sir0_data.content, sir0_data.data_pointer)
        } else {
            parse_wan_from_sir0_content(&sir0_data.content, sir0_data.data_pointer, wan_type)
        };

        parse_result.map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("WAN parse error: {:?}", e),
            )
        })
    }
}
