use std::fs::{self, File};
use std::io::{self, Read};
use std::io::{Cursor, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::containers::binpack::BinPack;
use crate::containers::compression::pkdpx::PkdpxContainer;
use crate::containers::sir0::{self};
use crate::containers::ContainerHandler;
use crate::data::animation_metadata::{AnimationType, MONSTER_BIN_ANIMS, M_ATTACK_BIN_ANIMS};
use crate::data::{MonsterEntry, MonsterStats, PokemonType, ShadowSize};
use crate::filesystem::{FileAllocationTable, FileNameTable};
use crate::graphics::wan::renderer::extract_frame;
use crate::graphics::wan::{parser, read_u16_le, WanFile};
use crate::graphics::WanType;
use crate::rom::read_header;

/// Direction names in order
const DIRECTIONS: &[&str] = &[
    "down",
    "down_right",
    "right",
    "up_right",
    "up",
    "up_left",
    "left",
    "down_left",
];

/// PokemonExtractor handles extracting Pokémon sprite data from the ROM
pub struct PokemonExtractor {
    rom_path: PathBuf,
    rom_data: Vec<u8>,
}

impl PokemonExtractor {
    /// Create a new extractor from a ROM file path
    pub fn new<P: AsRef<Path>>(rom_path: P) -> io::Result<Self> {
        let rom_path = rom_path.as_ref().to_path_buf();
        let mut rom_file = File::open(&rom_path)?;
        let mut rom_data = Vec::new();
        rom_file.read_to_end(&mut rom_data)?;

        Ok(PokemonExtractor { rom_path, rom_data })
    }

    /// Extract monster data, optionally limiting to specific Pokémon IDs
    pub fn extract_monster_data(
        &self,
        pokemon_ids: Option<&[usize]>,
        output_dir: &Path,
    ) -> io::Result<()> {
        // Read ROM header
        let header = read_header(&self.rom_path);

        // Parse FAT and FNT tables
        let fat =
            FileAllocationTable::read_from_rom(&self.rom_data, header.fat_offset, header.fat_size)?;

        let fnt = FileNameTable::read_from_rom(&self.rom_data, header.fnt_offset)?;

        // Get file IDs for monster.md, monster.bin, and m_attack.bin
        let monster_md_id = fnt
            .get_file_id("BALANCE/monster.md")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "monster.md not found"))?;

        let monster_bin_id = fnt
            .get_file_id("MONSTER/monster.bin")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "monster.bin not found"))?;

        let m_attack_bin_id = fnt
            .get_file_id("MONSTER/m_attack.bin")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "m_attack.bin not found"))?;

        // Extract file data
        let monster_md_data = fat
            .get_file_data(monster_md_id as usize, &self.rom_data)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Failed to extract monster.md")
            })?;

        let monster_bin_data = fat
            .get_file_data(monster_bin_id as usize, &self.rom_data)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Failed to extract monster.bin")
            })?;

        let m_attack_bin_data = fat
            .get_file_data(m_attack_bin_id as usize, &self.rom_data)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Failed to extract m_attack.bin")
            })?;

        // Parse monster.md
        println!("Parsing monster.md...");
        let monster_md = parse_monster_md(monster_md_data)?;

        // Parse bin files
        println!("Parsing monster.bin...");
        let monster_bin = BinPack::from_bytes(monster_bin_data).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse monster.bin BIN_PACK: {}", e),
            )
        })?;
        let m_attack_bin = BinPack::from_bytes(m_attack_bin_data).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse m_attack.bin BIN_PACK: {}", e),
            )
        })?;

        // Create output directory if it doesn't exist
        fs::create_dir_all(output_dir)?;

        // Determine which Pokémon IDs to process
        let ids_to_process = match pokemon_ids {
            Some(ids) => ids.to_vec(),
            None => (1..=10).collect(),
        };

        // Process the selected Pokémon IDs
        for id in ids_to_process {
            if id < monster_md.len() {
                let entry = &monster_md[id];
                // Process sprite data for this Pokémon if sprite index is valid
                let sprite_index = entry.sprite_index as usize;
                if sprite_index < monster_bin.len() && sprite_index < m_attack_bin.len() {
                    // Track if animation 11 has been processed already
                    let mut processed_anim_11 = false;

                    if let Err(e) = self.process_pokemon_sprite(
                        &monster_bin,
                        sprite_index,
                        "monster.bin",
                        &mut processed_anim_11,
                        output_dir,
                        id,
                    ) {
                        eprintln!(
                            "Error processing monster.bin sprite for Pokémon {}: {}",
                            id, e
                        );
                    }

                    if let Err(e) = self.process_pokemon_sprite(
                        &m_attack_bin,
                        sprite_index,
                        "m_attack.bin",
                        &mut processed_anim_11,
                        output_dir,
                        id,
                    ) {
                        eprintln!(
                            "Error processing m_attack.bin sprite for Pokémon {}: {}",
                            id, e
                        );
                    }
                } else {
                    println!(
                        "  - Invalid sprite index {} (out of range for bin files)",
                        entry.sprite_index
                    );
                }
            } else {
                println!("Pokémon ID {} is out of range", id);
            }
        }

        // Count Pokémon with valid sprite indexes
        let mut valid_sprites = 0;
        for (id, entry) in monster_md.iter().enumerate() {
            if id > 0 && entry.sprite_index < monster_bin.len() as i16 {
                valid_sprites += 1;
            }
        }

        Ok(())
    }

    /// Process an individual Pokémon sprite, handling decompression and parsing
    /// Returns the set of animation IDs processed
    fn process_pokemon_sprite(
        &self,
        bin_pack: &BinPack,
        sprite_index: usize,
        bin_name: &str,
        processed_anim_11: &mut bool,
        output_dir: &Path,
        pokemon_id: usize,
    ) -> io::Result<()> {
        // Extract sprite data from BIN_PACK
        let sprite_data = &bin_pack[sprite_index];

        // Detect compression type and decompress
        let decompressed_data = if sprite_data.starts_with(b"PKDPX") {
            self.decompress_pkdpx_data(sprite_data)?
        } else if sprite_data.starts_with(b"AT") {
            let format_id = String::from_utf8_lossy(&sprite_data[0..5]);
            return Ok(());
        } else {
            sprite_data.to_vec()
        };

        if decompressed_data.starts_with(b"SIR0") {
            match self.parse_sir0_to_wan(&decompressed_data) {
                Ok(wan_file) => {
                    self.analyse_wan_file(&wan_file, bin_name, processed_anim_11);

                    // Extract and save the sprite frames
                    // wan, pokemon_id, sprite_index, bin_name
                    if let Err(e) = self.extract_and_save_frames(
                        &wan_file,
                        pokemon_id,
                        sprite_index,
                        bin_name,
                        output_dir,
                        processed_anim_11,
                    ) {
                        println!("  - Error extracting frames: {}", e);
                    }
                }
                Err(e) => {
                    println!("  - Failed to parse SIR0 as WAN: {}", e);
                }
            }
        } else {
            println!("  - Decompressed data is not SIR0 format");
        }

        Ok(())
    }

    /// Extract and save frames from a WAN file
    fn extract_and_save_frames(
        &self,
        wan: &WanFile,
        pokemon_id: usize,
        sprite_index: usize,
        bin_name: &str,
        output_dir: &Path,
        processed_anim_11: &mut bool,
    ) -> io::Result<()> {
        // Create pokemon directory
        let pokemon_dir = output_dir.join(format!("pokemon_{:03}", pokemon_id));
        fs::create_dir_all(&pokemon_dir)?;

        // Process each animation group with its RAW index first
        for (group_idx, anim_group) in wan.animation_groups.iter().enumerate() {
            // Skip empty animation groups
            if anim_group.is_empty() {
                continue;
            }

            // Map the group index to a semantic animation ID - but ONLY for directory naming
            let anim_id = match bin_name {
                "monster.bin" => {
                    // Check if this group index corresponds to a known animation
                    if MONSTER_BIN_ANIMS.contains(&(group_idx as u8)) {
                        group_idx as u8 // Use group_idx directly as the animation ID
                    } else {
                        group_idx as u8
                    }
                }
                "m_attack.bin" => {
                    // For m_attack.bin, similar direct mapping
                    if M_ATTACK_BIN_ANIMS.contains(&(group_idx as u8)) {
                        group_idx as u8 // Use group_idx directly as the animation ID
                    } else {
                        group_idx as u8
                    }
                }
                _ => group_idx as u8,
            };

            // Skip animation 11 in m_attack.bin if it's already been processed
            if anim_id == 11 && bin_name == "m_attack.bin" && *processed_anim_11 {
                continue;
            }

            // If this is animation 11, mark it as processed
            if anim_id == 11 {
                *processed_anim_11 = true;
            }

            // Convert animation ID to type and name
            let anim_type = AnimationType::from(anim_id);
            let anim_name = anim_type.name();

            // Create animation directory
            let anim_dir = pokemon_dir.join(format!("anim_{:02}_{}", anim_id, anim_name));
            fs::create_dir_all(&anim_dir)?;

            // Sleep (ID 5) is single direction only
            let single_direction = anim_id == 5;

            // Process each direction
            let direction_count = if single_direction {
                1
            } else {
                anim_group.len().min(DIRECTIONS.len())
            };

            for dir_idx in 0..direction_count {
                if dir_idx >= anim_group.len() {
                    continue;
                }

                let anim_seq = &anim_group[dir_idx];
                let dir_name = DIRECTIONS[dir_idx];

                // Create direction directory
                let dir_dir = anim_dir.join(dir_name);
                fs::create_dir_all(&dir_dir)?;

                // Track successful and failed frames
                let mut successful_frames = 0;
                let mut empty_frames = 0;
                let mut failed_frames = 0;

                // Process each frame with better error handling
                for (frame_idx, anim_frame) in anim_seq.frames.iter().enumerate() {
                    let frame_id = anim_frame.frame_index as usize;

                    // Skip if frame doesn't exist
                    if frame_id >= wan.frame_data.len() {
                        println!("        - Invalid frame index {} - skipping", frame_id);
                        failed_frames += 1;
                        continue;
                    }

                    // Extract the frame - using renderer
                    match extract_frame(wan, frame_id) {
                        Ok(frame_img) => {
                            // Count non-transparent pixels for troubleshooting
                            let non_transparent_count =
                                frame_img.pixels().filter(|p| p[3] > 0).count();

                            if non_transparent_count == 0 {
                                empty_frames += 1;
                            }

                            // Save the raw frame directly
                            let raw_path = dir_dir.join(format!("frame_{:02}_raw.png", frame_idx));
                            if let Err(e) = frame_img.save(&raw_path) {
                                println!("        - Error saving raw frame: {}", e);
                                failed_frames += 1;
                            } else {
                                successful_frames += 1;
                            }
                        }
                        Err(e) => {
                            println!("        - Error extracting frame {}: {:?}", frame_id, e);
                            failed_frames += 1;
                        }
                    }
                }
            }
        }

        Ok(())
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
        // Parse SIR0 container with enhanced error reporting
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

        // Create a cursor to read the WAN header
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

        // Determine WAN type based on image type
        let wan_type = match img_type {
            1 => WanType::Character,
            2 | 3 => WanType::Effect,
            _ => {
                // For unknown types, log a warning and default to Character
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

    /// Analyze a WAN file and print information about its contents
    /// Takes a mutable reference to processed_anim_11 to track animation 11 processing
    fn analyse_wan_file(&self, wan: &WanFile, bin_name: &str, processed_anim_11: &mut bool) {
        // First print raw animation group structure
        for (i, group) in wan.animation_groups.iter().enumerate() {
            let group_size = group.len();
            if group_size > 0 {
                // Calculate total frames
                let total_frames: usize = group.iter().map(|anim| anim.frames.len()).sum();
            }
        }

        match bin_name {
            "monster.bin" => {
                for (group_idx, group) in wan.animation_groups.iter().enumerate() {
                    if !group.is_empty() {
                        // Use direct mapping: check if group_idx is a known animation ID
                        let anim_id = if MONSTER_BIN_ANIMS.contains(&(group_idx as u8)) {
                            group_idx as u8
                        } else {
                            255 // Unknown
                        };

                        // Skip animation 11 if we've already processed it and this is m_attack.bin
                        if anim_id == 11 {
                            *processed_anim_11 = true;
                        }

                        let anim_type = AnimationType::from(anim_id);
                    }
                }
            }
            "m_attack.bin" => {
                for (group_idx, group) in wan.animation_groups.iter().enumerate() {
                    if !group.is_empty() {
                        let anim_id = if M_ATTACK_BIN_ANIMS.contains(&(group_idx as u8)) {
                            group_idx as u8
                        } else {
                            255 // Unknown
                        };

                        if anim_id == 11 && *processed_anim_11 {
                            continue;
                        }

                        if anim_id == 11 {
                            *processed_anim_11 = true;
                        }

                        let anim_type = AnimationType::from(anim_id);
                    }
                }
            }
            _ => {
                println!("    Unknown bin file: {}", bin_name);
            }
        }

        // Analyze frame data for debugging
        let mut empty_frames = 0;
        let mut total_pieces = 0;
        let mut minus_frame_refs = 0;

        for (i, frame) in wan.frame_data.iter().enumerate() {
            total_pieces += frame.pieces.len();
            if frame.pieces.is_empty() {
                empty_frames += 1;
            }

            for piece in &frame.pieces {
                if piece.img_index < 0 {
                    minus_frame_refs += 1;
                }
            }
        }
    }
}

/// Parse the monster.md file to extract monster entries
fn parse_monster_md(data: &[u8]) -> io::Result<Vec<MonsterEntry>> {
    // Check magic number for MD format: "MD\0\0"
    if data.len() < 8 || &data[0..4] != b"MD\0\0" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid monster.md format: missing MD\\0\\0 magic number",
        ));
    }

    // Read number of entries
    let number_entries = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    println!("Found {} entries in monster.md", number_entries);

    // Create a vector to hold all monster entries
    let mut entries = Vec::with_capacity(number_entries);

    // Parse each entry (each entry is 68 bytes)
    const ENTRY_SIZE: usize = 68;

    for i in 0..number_entries {
        let start = 8 + (i * ENTRY_SIZE); // 8 bytes for header

        if start + ENTRY_SIZE > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Monster entry {} out of bounds", i + 1),
            ));
        }

        // Extract national_pokedex_number (offset 0x04)
        let national_pokedex_number = u16::from_le_bytes([data[start + 0x04], data[start + 0x05]]);

        // Extract sprite_index (offset 0x10)
        let sprite_index = i16::from_le_bytes([data[start + 0x10], data[start + 0x11]]);

        // Create an entry with default values for other fields
        entries.push(MonsterEntry {
            md_index: i as u32,
            national_pokedex_number,
            sprite_index,
            // Use default values for other fields until we need them
            stats: MonsterStats {
                base_hp: 0,
                base_atk: 0,
                base_sp_atk: 0,
                base_def: 0,
                base_sp_def: 0,
            },
            type_primary: PokemonType::None,
            type_secondary: PokemonType::None,
            weight: 0,
            shadow_size: ShadowSize::Medium,
        });
    }
    Ok(entries)
}
