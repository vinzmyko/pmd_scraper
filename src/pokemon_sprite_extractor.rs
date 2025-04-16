use std::{
    collections::HashMap,
    fs::{self},
    io::{self, Cursor, Seek, SeekFrom},
    path::Path,
};

use crate::{
    containers::{
        binpack::BinPack,
        compression::pkdpx::PkdpxContainer,
        sir0::{self},
        ContainerHandler,
    },
    data::{monster_md::MonsterData, MonsterEntry},
    graphics::{
        atlas::{create_pokemon_atlas, AtlasConfig},
        wan::{parser, WanFile},
        WanType,
    },
    rom::Rom,
    binary_utils::read_u16_le,
};

/// Handles extracting Pokémon sprite data from the ROM
pub struct PokemonExtractor<'a> {
    rom: &'a Rom,
}

impl<'a> PokemonExtractor<'a> {
    pub fn new(rom: &'a Rom) -> Self {
        PokemonExtractor { rom }
    }

    pub fn extract_monster_data(
        &self,
        pokemon_ids: Option<&[usize]>,
        output_dir: &Path,
    ) -> io::Result<()> {
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
        let monster_bin = BinPack::from_bytes(monster_bin_data).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse monster.bin BIN_PACK: {}", e),
            )
        })?;

        println!("Parsing m_attack.bin...");
        let m_attack_bin = BinPack::from_bytes(m_attack_bin_data).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse m_attack.bin BIN_PACK: {}", e),
            )
        })?;

        fs::create_dir_all(output_dir)?;

        let ids_to_process = match pokemon_ids {
            Some(ids) => ids.to_vec(),
            None => {
                println!("Processing all Pokémon found in monster.md...");
                (1..monster_md.len()).collect()
            }
        };

        let atlas_config = AtlasConfig::default();

        for id in ids_to_process {
            if id < monster_md.len() {
                let entry = &monster_md[id];
                self.process_pokemon(
                    id,
                    entry,
                    &monster_bin,
                    &m_attack_bin,
                    &atlas_config,
                    output_dir,
                )?;
            } else {
                println!("Pokémon ID {} is out of range", id);
            }
        }

        Ok(())
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
                format!("AT format not supported for WAN extraction"),
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

    /// Process a single Pokémon's sprite data
    fn process_pokemon(
        &self,
        id: usize,
        entry: &MonsterEntry,
        monster_bin: &BinPack,
        m_attack_bin: &BinPack,
        atlas_config: &AtlasConfig,
        output_dir: &Path,
    ) -> io::Result<()> {
        let sprite_index = entry.sprite_index as usize;
        if sprite_index < monster_bin.len() && sprite_index < m_attack_bin.len() {
            println!(
                "Processing Pokémon #{:03} (Sprite Index: {})",
                id, sprite_index
            );

            let mut wan_files: HashMap<String, WanFile> = HashMap::new();

            if let Ok(wan_file) = self.extract_wan_file(monster_bin, sprite_index) {
                wan_files.insert("monster.bin".to_string(), wan_file);
            }

            if let Ok(wan_file) = self.extract_wan_file(m_attack_bin, sprite_index) {
                wan_files.insert("m_attack.bin".to_string(), wan_file);
            }

            // If we have valid WAN files, generate atlas
            if !wan_files.is_empty() {
                println!("Generating sprite atlas for Pokémon #{:03}...", id);

                let dex_num = entry.national_pokedex_number;

                match create_pokemon_atlas(&wan_files, id, dex_num, atlas_config, output_dir) {
                    Ok(atlas_result) => {
                        println!(
                            "Successfully generated atlas at: {}",
                            atlas_result.image_path.display()
                        );
                        println!(
                            "Metadata saved at: {}",
                            atlas_result.metadata_path.display()
                        );
                        println!(
                            "Atlas dimensions: {}x{}, Frame size: {}x{}",
                            atlas_result.dimensions.0,
                            atlas_result.dimensions.1,
                            atlas_result.frame_dimensions.0,
                            atlas_result.frame_dimensions.1
                        );
                    }
                    Err(e) => {
                        eprintln!("Error generating atlas for Pokémon #{:03}: {:?}", id, e);
                    }
                }
            } else {
                println!("No valid WAN files found for Pokémon #{:03}", id);
            }
        } else {
            println!(
                "  - Invalid sprite index {} (out of range for bin files)",
                entry.sprite_index
            );
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
