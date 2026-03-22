/// Extract move names from the text_e.str string table.
///
/// Move names are stored sequentially (by move ID order) in different regions of the string table
/// depending on the game version. The indices below define the start and end positions for each version.
///
/// # String Block Indices for Move Names
///
/// ## Explorers of Sky (North America)
/// - Game IDs: `EoS_NA`, `EoSWVC_NA`
/// - Begin Index: **8173**
/// - End Index: **8734**
/// - Total Moves: 561
///
/// ## Explorers of Sky (Europe)
/// - Game IDs: `EoS_EU`, `EoSWVC_EU`
/// - Begin Index: **8175**
/// - End Index: **8736**
/// - Total Moves: 561
///
/// ## Explorers of Sky (Japan)
/// - Game ID: `EoS_JP`
/// - Begin Index: **4874**
/// - End Index: **5435**
/// - Total Moves: 561
///
/// # Notes
/// - Currently hardcoded for EoS NA - update `MOVE_NAMES_BEGIN` constant for other regions
/// - The string table also contains an alphabetical section (used for in-game menus)
///   which should NOT be used for move ID mapping
use std::{
    collections::HashMap,
    fs::File,
    io::{self, Cursor},
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::{
    binary_utils::{read_u16_le, read_u32_le, read_u8},
    containers::sir0::Sir0,
    rom::Rom,
};

fn move_type_str(value: u8) -> String {
    match value {
        0 => "None",
        1 => "Normal",
        2 => "Fire",
        3 => "Water",
        4 => "Grass",
        5 => "Electric",
        6 => "Ice",
        7 => "Fighting",
        8 => "Poison",
        9 => "Ground",
        10 => "Flying",
        11 => "Psychic",
        12 => "Bug",
        13 => "Rock",
        14 => "Ghost",
        15 => "Dragon",
        16 => "Dark",
        17 => "Steel",
        18 => "Neutral",
        _ => "Unknown",
    }
    .to_string()
}

fn move_category_str(value: u8) -> String {
    match value {
        0 => "Physical",
        1 => "Special",
        2 => "Status",
        3 => "None",
        _ => "Unknown",
    }
    .to_string()
}

fn move_target_str(value: u8) -> String {
    match value {
        0 => "Enemies",
        1 => "Party (including user)",
        2 => "All (including user)",
        3 => "User",
        4 => "Enemies (after charging)",
        5 => "All (except user)",
        6 => "Teammates (excluding user)",
        15 => "Special (unique targeting, e.g. Spikes, Curse)",
        _ => "Unknown",
    }
    .to_string()
}

fn move_range_str(value: u8) -> String {
    match value {
        0 => "Front (1 tile)",
        1 => "Front_and_Sides (cuts corners)",
        2 => "Nearby (8 surrounding tiles)",
        3 => "Room",
        4 => "Front_2 (2 tiles, cuts corners, AI unaware)",
        5 => "Front_10 (10 tiles)",
        6 => "Floor",
        7 => "User (self-target or front after charging)",
        8 => "Front (cuts corners)",
        9 => "Front_2 (2 tiles, cuts corners, AI aware)",
        15 => "Special (unique range, move-specific behavior)",
        _ => "Unknown",
    }
    .to_string()
}

fn ai_condition_str(value: u8) -> String {
    match value {
        0 => "Always eligible",
        1 => "Random chance (uses ai_random_use_chance %)",
        2 => "Target HP <= 25%",
        3 => "Target has negative status",
        4 => "Target is asleep/napping/nightmare",
        5 => "Target HP <= 25% or has negative status",
        6 => "Target is Ghost type and not exposed",
        _ => "Unknown",
    }
    .to_string()
}

/// Unpack a 16-bit target_range field into (target, range, ai_condition)
fn unpack_target_range(packed: u16) -> (u8, u8, u8) {
    let target = (packed & 0x0F) as u8;
    let range = ((packed >> 4) & 0x0F) as u8;
    let ai_condition = ((packed >> 8) & 0x0F) as u8;
    (target, range, ai_condition)
}

/// Represents a single move entry from waza_p.bin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveData {
    pub move_id: u16,
    pub name: String,
    pub base_power: u16,
    pub move_type: String,
    pub category: String,
    pub target: String,
    pub range: String,

    pub ai_target: String,
    pub ai_range: String,
    pub ai_use_condition: String,
    pub ai_random_use_chance: u8,
    pub ai_weight: u8,
    pub ai_can_use_against_frozen: bool,

    pub pp: u8,
    pub accuracy1: u8,
    pub accuracy2: u8,
    pub strikes: u8,
    pub crit_chance: u8,
    pub max_ginseng_boost: u8,
    // Interaction flags
    pub reflected_by_magic_coat: bool,
    pub can_be_snatched: bool,
    pub fails_while_muzzled: bool,
    pub usable_while_taunted: bool,
    pub message_string_idx: u16,
}

/// Main extractor struct
pub struct MoveDataExtractor<'a> {
    rom: &'a Rom,
}

impl<'a> MoveDataExtractor<'a> {
    pub fn new(rom: &'a Rom) -> Self {
        MoveDataExtractor { rom }
    }

    /// Extract move data and save to JSON files
    pub fn extract_and_save(&self, output_dir: &Path) -> io::Result<()> {
        println!("Starting move data extraction...");

        println!("  Loading text_e.str for move names...");
        let move_names = self.load_move_names()?;
        println!("  Loaded {} move names", move_names.len());

        let waza_data = self.load_waza_p_bin()?;

        let sir0_data = Sir0::from_bytes(&waza_data)?;

        let moves = self.parse_move_data(&sir0_data, &move_names)?;

        println!("  Extracted {} moves", moves.len());

        self.save_move_lookup(&moves, output_dir)?;
        self.save_move_data(&moves, output_dir)?;

        println!("Move data extraction complete!");
        Ok(())
    }

    /// Load move names from text_e.str
    fn load_move_names(&self) -> io::Result<Vec<String>> {
        let possible_paths = [
            "MESSAGE/text_e.str",
            "MESSAGE/text_e.bin",
            "MESSAGE/text_j.str",
            "MESSAGE/text_j.bin",
        ];

        let text_data = possible_paths
            .iter()
            .find_map(|&path| {
                self.rom
                    .fnt
                    .get_file_id(path)
                    .and_then(|id| self.rom.fat.get_file_data(id as usize, &self.rom.data))
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "Could not find text_e.str or text_j.str in ROM",
                )
            })?;

        println!("  Found text file: {} bytes", text_data.len());

        let strings = self.parse_string_table(text_data)?;
        println!("  Parsed {} total strings from text file", strings.len());

        let move_names = self.extract_move_names_from_strings(&strings)?;

        Ok(move_names)
    }

    /// Parse the text_*.str string table format
    fn parse_string_table(&self, data: &[u8]) -> io::Result<Vec<String>> {
        let mut cursor = Cursor::new(data);
        let mut pointers = Vec::new();

        loop {
            if cursor.position() as usize + 4 > data.len() {
                break;
            }

            let ptr = read_u32_le(&mut cursor)?;

            if ptr as usize >= data.len() {
                pointers.push(ptr);
                break;
            }

            pointers.push(ptr);

            if ptr == cursor.position() as u32 {
                break;
            }
        }

        println!("  Found {} string pointers", pointers.len());

        let mut strings = Vec::with_capacity(pointers.len());

        for i in 0..pointers.len() - 1 {
            let start = pointers[i] as usize;
            let end = pointers[i + 1] as usize;

            if start >= data.len() || end > data.len() || start >= end {
                strings.push(String::new());
                continue;
            }

            let string_data = &data[start..end];
            let null_pos = string_data
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(string_data.len());
            let str_bytes = &string_data[..null_pos];

            let text = String::from_utf8_lossy(str_bytes).to_string();
            strings.push(text);
        }

        Ok(strings)
    }

    fn extract_move_names_from_strings(&self, strings: &[String]) -> io::Result<Vec<String>> {
        const MOVE_NAMES_BEGIN: usize = 8173;
        const MOVE_NAMES_END: usize = 8734;

        if strings.len() < MOVE_NAMES_END {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "String table too small. Expected at least {} strings, got {}",
                    MOVE_NAMES_END,
                    strings.len()
                ),
            ));
        }

        let move_names = strings[MOVE_NAMES_BEGIN..MOVE_NAMES_END].to_vec();

        Ok(move_names)
    }

    /// Load waza_p.bin from ROM
    fn load_waza_p_bin(&self) -> io::Result<Vec<u8>> {
        let file_id = self
            .rom
            .fnt
            .get_file_id("BALANCE/waza_p.bin")
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "waza_p.bin not found in ROM")
            })?;

        self.rom
            .fat
            .get_file_data(file_id as usize, &self.rom.data)
            .map(|data| data.to_vec())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Failed to extract waza_p.bin data",
                )
            })
    }

    /// Parse move data from SIR0 content
    fn parse_move_data(&self, sir0: &Sir0, move_names: &[String]) -> io::Result<Vec<MoveData>> {
        let mut cursor = Cursor::new(&sir0.content as &[u8]);

        cursor.set_position(sir0.data_pointer as u64);

        let ptr_moves_data = read_u32_le(&mut cursor)?;
        let _ptr_moveset_table = read_u32_le(&mut cursor)?;

        println!("  Moves data pointer: 0x{:X}", ptr_moves_data);

        cursor.set_position(ptr_moves_data as u64);

        let mut moves = Vec::new();
        let mut move_index = 0;

        loop {
            let current_pos = cursor.position() as usize;

            if current_pos + 26 > sir0.content.len() {
                break;
            }

            if sir0.content[current_pos] == 0xAA {
                println!("  Found padding marker at offset 0x{:X}", current_pos);
                break;
            }

            let name = move_names
                .get(move_index)
                .cloned()
                .unwrap_or_else(|| format!("Unknown_{:04}", move_index));

            let move_data = self.parse_move_entry(&mut cursor, name)?;
            moves.push(move_data);

            move_index += 1;
        }

        Ok(moves)
    }

    /// Parse a single 26-byte move entry
    fn parse_move_entry(&self, cursor: &mut Cursor<&[u8]>, name: String) -> io::Result<MoveData> {
        let base_power = read_u16_le(cursor)?; // 0x00
        let raw_type = read_u8(cursor)?; // 0x02
        let raw_category = read_u8(cursor)?; // 0x03
        let raw_target_range = read_u16_le(cursor)?; // 0x04
        let raw_ai_target_range = read_u16_le(cursor)?; // 0x06
        let pp = read_u8(cursor)?; // 0x08
        let ai_weight = read_u8(cursor)?; // 0x09
        let accuracy1 = read_u8(cursor)?; // 0x0A
        let accuracy2 = read_u8(cursor)?; // 0x0B
        let ai_random_use_chance = read_u8(cursor)?; // 0x0C
        let strikes = read_u8(cursor)?; // 0x0D
        let max_ginseng_boost = read_u8(cursor)?; // 0x0E
        let crit_chance = read_u8(cursor)?; // 0x0F
        let reflected_by_magic_coat = read_u8(cursor)? != 0; // 0x10
        let can_be_snatched = read_u8(cursor)? != 0; // 0x11
        let fails_while_muzzled = read_u8(cursor)? != 0; // 0x12
        let ai_can_use_against_frozen = read_u8(cursor)? != 0; // 0x13
        let usable_while_taunted = read_u8(cursor)? != 0; // 0x14
        let _range_string_idx = read_u8(cursor)?; // 0x15
        let move_id = read_u16_le(cursor)?; // 0x16
        let message_string_idx = read_u16_le(cursor)?; // 0x18

        let (target_val, range_val, _ai_cond_val) = unpack_target_range(raw_target_range);
        let (ai_target_val, ai_range_val, ai_cond_val) = unpack_target_range(raw_ai_target_range);

        Ok(MoveData {
            move_id,
            name,
            base_power,
            move_type: move_type_str(raw_type),
            category: move_category_str(raw_category),
            target: move_target_str(target_val),
            range: move_range_str(range_val),
            ai_target: move_target_str(ai_target_val),
            ai_range: move_range_str(ai_range_val),
            ai_use_condition: ai_condition_str(ai_cond_val),
            ai_random_use_chance,
            ai_weight,
            ai_can_use_against_frozen,
            pp,
            accuracy1,
            accuracy2,
            strikes,
            crit_chance,
            max_ginseng_boost,
            reflected_by_magic_coat,
            can_be_snatched,
            fails_while_muzzled,
            usable_while_taunted,
            message_string_idx,
        })
    }

    /// Save move lookup JSON (name -> ID mapping) with snake_case keys
    fn save_move_lookup(&self, moves: &[MoveData], output_dir: &Path) -> io::Result<()> {
        fn to_snake_case(s: &str) -> String {
            let mut result = String::with_capacity(s.len() + 3);

            for (i, ch) in s.chars().enumerate() {
                match ch {
                    ' ' | '-' => result.push('_'),
                    '\'' => continue,
                    c if c.is_uppercase() => {
                        if i > 0 && !result.ends_with('_') {
                            result.push('_');
                        }
                        result.push(c.to_ascii_lowercase());
                    }
                    _ => result.push(ch.to_ascii_lowercase()),
                }
            }

            result
        }

        let lookup: HashMap<String, u16> = moves
            .iter()
            .enumerate()
            .map(|(idx, m)| (to_snake_case(&m.name), idx as u16))
            .collect();

        let output_path = output_dir.join("move_lookup.json");
        let file = File::create(&output_path)?;

        serde_json::to_writer_pretty(file, &lookup)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        println!("  Saved move lookup to {}", output_path.display());
        Ok(())
    }

    /// Save full move data JSON
    fn save_move_data(&self, moves: &[MoveData], output_dir: &Path) -> io::Result<()> {
        let output_path = output_dir.join("move_data.json");
        let file = File::create(&output_path)?;

        let move_map: HashMap<u16, &MoveData> = moves.iter().map(|m| (m.move_id, m)).collect();

        serde_json::to_writer_pretty(file, &move_map)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        println!("  Saved move data to {}", output_path.display());
        Ok(())
    }
}
