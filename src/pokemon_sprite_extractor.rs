use std::{
    collections::HashMap,
    fs::{self},
    io::{self, Cursor, Seek, SeekFrom},
    path::Path,
};

use crate::{
    binary_utils::read_u16_le,
    containers::{
        binpack::BinPack,
        compression::pkdpx::PkdpxContainer,
        sir0::{self},
        ContainerHandler,
    },
    data::{monster_md::MonsterData, MonsterEntry},
    graphics::{
        atlas::{create_pokemon_atlas, AtlasConfig},
        wan::{parser, Animation, AnimationStructure, WanFile},
        WanType,
    },
    progress::write_progress,
    rom::Rom,
};

/// Groups shared data and configuration for processing multiple Pokémon
struct PokemonProcessingContext<'a> {
    monster_bin: &'a BinPack,
    m_attack_bin: &'a BinPack,
    atlas_config: &'a AtlasConfig,
    output_dir: &'a Path,
    all_entries: &'a [MonsterEntry],
}

/// Handles extracting Pokemon sprite data from the ROM
pub struct PokemonSpriteExtractor<'a> {
    rom: &'a Rom,
}

impl<'a> PokemonSpriteExtractor<'a> {
    pub fn new(rom: &'a Rom) -> Self {
        PokemonSpriteExtractor { rom }
    }

    pub fn extract_monster_data(
        &self,
        pokemon_ids: Option<u32>,
        output_dir: &Path,
        progress_path: &Path,
    ) -> io::Result<()> {
        // Load all necessary data files
        let monster_md_id = self
            .rom
            .fnt
            .get_file_id("BALANCE/monster.md")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "monster.md not found"))?;
        let monster_bin_id = self
            .rom
            .fnt
            .get_file_id("MONSTER/monster.bin")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "monster.bin not found"))?;
        let m_attack_bin_id = self
            .rom
            .fnt
            .get_file_id("MONSTER/m_attack.bin")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "m_attack.bin not found"))?;
        let monster_md_data = self
            .rom
            .fat
            .get_file_data(monster_md_id as usize, &self.rom.data)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Failed to extract monster.md")
            })?;
        let monster_bin_data = self
            .rom
            .fat
            .get_file_data(monster_bin_id as usize, &self.rom.data)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Failed to extract monster.bin")
            })?;
        let m_attack_bin_data = self
            .rom
            .fat
            .get_file_data(m_attack_bin_id as usize, &self.rom.data)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Failed to extract m_attack.bin")
            })?;

        println!("Parsing monster.md...");
        let monster_md = parse_monster_md(monster_md_data)?;
        println!("Parsing monster.bin...");
        let monster_bin = BinPack::from_bytes(monster_bin_data)?;
        println!("Parsing m_attack.bin...");
        let m_attack_bin = BinPack::from_bytes(m_attack_bin_data)?;
        fs::create_dir_all(output_dir)?;

        // Build the definitive list of entries to process
        let final_list: Vec<(usize, String)>;

        // make it num_pokemon
        if let Some(ids) = pokemon_ids {
            let mut list = Vec::new();
            for id in 0..=ids {
                let entry = &monster_md[id as usize];
                let folder_name = if id == 537 {
                    "pokemon_000".to_string()
                } else {
                    format!("pokemon_{:03}", entry.national_pokedex_number)
                };
                list.push((id as usize, folder_name));
            }
            final_list = list;
        } else {
            println!("Filtering all monster.md entries to find useful sprites...");
            let mut list = Vec::new();
            let mut form_counts: std::collections::HashMap<u16, u16> =
                std::collections::HashMap::new();
            const SUBSTITUTE_DOLL_MD_INDEX: usize = 537;

            for i in 0..monster_md.len() {
                let entry = &monster_md[i];
                let dex_num = entry.national_pokedex_number;
                let mut is_generic_form = false;
                let mut folder_name = format!("pokemon_{:03}", dex_num);

                if i < 600 {
                    let form_id = *form_counts.entry(dex_num).or_insert(0);

                    if form_id > 0 && i != SUBSTITUTE_DOLL_MD_INDEX {
                        if let Some(form_name) = self.get_form_name(dex_num, form_id) {
                            folder_name.push_str(&format!("_{}", form_name));
                        } else {
                            folder_name.push_str(&format!("_form_{}", form_id));
                            if dex_num > 0 {
                                is_generic_form = true;
                            }
                        }
                    }
                    *form_counts.entry(dex_num).or_default() += 1;
                } else {
                    let primary_index = i - 600;
                    if primary_index < monster_md.len() {
                        let primary_entry = &monster_md[primary_index];
                        if primary_entry.sprite_index != entry.sprite_index && entry.gender == 2 {
                            folder_name.push_str("_f");
                        }
                    }
                }

                let should_keep =
                    i == SUBSTITUTE_DOLL_MD_INDEX || (dex_num > 0 && !is_generic_form);

                if should_keep {
                    list.push((i, folder_name));
                }
            }
            //final_list = list;
            //let mut folder_name = format!("pokemon_{:03}", dex_num);
            let pikachu: (usize, String) = (25 as usize, "pokemon_025".to_string());
            final_list = vec![pikachu];
        }

        println!("Found {} useful entries to process.", final_list.len());
        let atlas_config = AtlasConfig::default();
        let context = PokemonProcessingContext {
            monster_bin: &monster_bin,
            m_attack_bin: &m_attack_bin,
            atlas_config: &atlas_config,
            output_dir,
            all_entries: &monster_md,
        };

        // Process the clean filtered list
        for (i, (id, folder_name)) in final_list.iter().enumerate() {
            let entry = &monster_md[*id];
            self.process_pokemon(*id, entry, &folder_name, &context)?;
            write_progress(
                progress_path,
                i + 1,
                final_list.len(),
                "pokemon_sprite",
                "running",
            );
        }

        Ok(())
    }

    /// Get a human-readable form name if applicable
    fn get_form_name(&self, dex_num: u16, form_index: u16) -> Option<String> {
        match dex_num {
            201 => {
                // Unown forms: A-Z, !, ?
                match form_index {
                    0 => Some("a".to_string()),
                    1..=25 => Some(((b'a' + form_index as u8) as char).to_string()),
                    26 => Some("exclamation".to_string()),
                    27 => Some("question".to_string()),
                    _ => None,
                }
            }
            351 => {
                // Castform forms
                match form_index {
                    0 => None, // Base form
                    1 => Some("snowy".to_string()),
                    2 => Some("sunny".to_string()),
                    3 => Some("rainy".to_string()),
                    _ => None,
                }
            }

            386 => {
                // Deoxys forms
                match form_index {
                    0 => Some("normal".to_string()),
                    1 => Some("attack".to_string()),
                    2 => Some("defense".to_string()),
                    3 => Some("speed".to_string()),
                    _ => None,
                }
            }
            412 | 413 => {
                // Burmy/Wormadam forms
                match form_index {
                    0 => Some("sandy".to_string()),
                    1 => Some("plant".to_string()),
                    2 => Some("trash".to_string()),
                    _ => None,
                }
            }
            421 => {
                // Cherrim forms
                match form_index {
                    0 => Some("overcast".to_string()),
                    1 => Some("sunshine".to_string()),
                    _ => None,
                }
            }
            422 | 423 => {
                // Shellos/Gastrodon forms
                match form_index {
                    0 => Some("west".to_string()),
                    1 => Some("east".to_string()),
                    _ => None,
                }
            }
            479 => {
                // Rotom forms do not exist in PMD: EoS
                // Forms only added in Pokemon Platinum before Time/Darkness not added in Sky
                match form_index {
                    0 => None, // Base form
                    _ => None,
                }
            }
            483 => {
                // Dialga forms
                match form_index {
                    0 => None, // Base form
                    1 => Some("primal".to_string()),
                    _ => None,
                }
            }
            487 => {
                // Giratina forms
                match form_index {
                    0 => Some("altered".to_string()),
                    1 => Some("origin".to_string()),
                    _ => None,
                }
            }
            492 => {
                // Shaymin forms
                match form_index {
                    0 => Some("land".to_string()),
                    1 => Some("sky".to_string()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Extract a WAN file from a bin file
    fn extract_wan_file(&self, bin_pack: &BinPack, sprite_index: usize) -> io::Result<WanFile> {
        let sprite_data = &bin_pack[sprite_index];

        // Detect compression type and decompress
        let decompressed_data = if sprite_data.starts_with(b"PKDPX") {
            self.decompress_pkdpx_data(sprite_data)?
        } else if sprite_data.starts_with(b"AT") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "AT format not supported for WAN extraction".to_string(),
            ));
        } else {
            sprite_data.to_vec()
        };

        if decompressed_data.starts_with(b"SIR0") {
            self.parse_sir0_to_wan(&decompressed_data)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Decompressed data is not SIR0 format",
            ))
        }
    }

    /// Decompress data from a PKDPX container
    fn decompress_pkdpx_data(&self, data: &[u8]) -> io::Result<Vec<u8>> {
        match PkdpxContainer::deserialise(data) {
            Ok(pkdpx) => match pkdpx.decompress() {
                Ok(decompressed) => Ok(decompressed),
                Err(e) => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to decompress PKDPX: {}", e),
                )),
            },
            Err(e) => Err(e),
        }
    }

    /// Parse a SIR0 container and extract WAN file
    fn parse_sir0_to_wan(&self, data: &[u8]) -> io::Result<WanFile> {
        let sir0_data = match sir0::Sir0::from_bytes(data) {
            Ok(sir0) => sir0,
            Err(e) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to parse SIR0: {}", e),
                ));
            }
        };

        // Validate data_pointer
        if sir0_data.data_pointer as usize >= sir0_data.content.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid data_pointer: 0x{:x} (content length: {})",
                    sir0_data.data_pointer,
                    sir0_data.content.len()
                ),
            ));
        }

        let mut reader = Cursor::new(&sir0_data.content[..]);

        // Seek to the data pointer position with bounds checking
        match reader.seek(SeekFrom::Start(sir0_data.data_pointer as u64)) {
            Ok(_) => {}
            Err(e) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to seek to data pointer: {}", e),
                ));
            }
        }

        // Skip the pointers to AnimInfo and ImageDataInfo (8 bytes)
        match reader.seek(SeekFrom::Current(8)) {
            Ok(_) => {}
            Err(e) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to skip pointers in WAN header: {}", e),
                ));
            }
        }

        // Read the image type to determine WAN type
        let img_type = match read_u16_le(&mut reader) {
            Ok(val) => val,
            Err(e) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to read image type: {}", e),
                ));
            }
        };

        let wan_type = match img_type {
            1 => WanType::Character,
            2 | 3 => WanType::Effect,
            _ => {
                println!(
                    "  - Unknown WAN image type: {}, defaulting to Character",
                    img_type
                );
                WanType::Character
            }
        };

        parser::parse_wan_from_sir0_content(
            &sir0_data.content[..],
            sir0_data.data_pointer,
            wan_type,
        )
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse WAN: {:?}", e),
            )
        })
    }

    /// Process a single Pokemon's sprite data
    fn process_pokemon(
        &self,
        id: usize,
        entry: &MonsterEntry,
        folder_name: &str,
        context: &PokemonProcessingContext,
    ) -> io::Result<()> {
        // De-duplicate visually identical gender variants
        if id >= 600 {
            let primary_index = id - 600;
            if primary_index < context.all_entries.len() {
                let primary_entry = &context.all_entries[primary_index];
                if primary_entry.sprite_index == entry.sprite_index {
                    return Ok(());
                }
            }
        }

        let sprite_index = entry.sprite_index as usize;
        if sprite_index >= context.monster_bin.len() || sprite_index >= context.m_attack_bin.len() {
            println!(
                "Skipping Pokemon #{:03} ('{}'): Invalid sprite index {}",
                id, folder_name, sprite_index
            );
            return Ok(());
        }

        // Extract and log pre-merge stats
        let monster_wan = self.extract_wan_file(context.monster_bin, sprite_index)?;
        let attack_wan = self.extract_wan_file(context.m_attack_bin, sprite_index)?;

        let mut wan_files = HashMap::new();
        // wan_files.insert("monster".to_string(), monster_wan);
        wan_files.insert("m_attack".to_string(), attack_wan);

        println!("Generating sprite atlas for {}...", folder_name);

        match create_pokemon_atlas(
            &wan_files,
            id,
            entry.national_pokedex_number,
            context.atlas_config,
            context.output_dir,
            folder_name,
        ) {
            Ok(atlas_result) => {
                println!(
                    "  -> Successfully generated atlas at: {}",
                    atlas_result.image_path.display()
                );
            }
            Err(e) => {
                eprintln!("  -> Error generating atlas for {}: {:?}", folder_name, e);
            }
        }
        Ok(())
    }
}

/// Parse the monster.md file to extract monster entries
fn parse_monster_md(data: &[u8]) -> io::Result<Vec<MonsterEntry>> {
    // Use the more comprehensive parser from monster_md.rs
    let monster_data = MonsterData::parse(data)?;

    // Log the entry count (to maintain the same output as before)
    println!("Found {} entries in monster.md", monster_data.entries.len());

    // Return the entries directly
    Ok(monster_data.entries)
}
