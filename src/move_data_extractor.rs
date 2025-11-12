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

/// Represents a single move entry from waza_p.bin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveData {
    pub move_id: u16,
    pub name: String,
    pub base_power: u16,
    pub move_type: u8,
    pub category: u8,
    pub base_pp: u8,
    pub accuracy: u8,
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
        // Try different possible paths for the text file
        let possible_paths = [
            "MESSAGE/text_e.str",
            "MESSAGE/text_e.bin",
            "MESSAGE/text_j.str", // Japanese version
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

        // Extract move names from the string table
        // Move names start at different indices depending on the game version
        // For Explorers of Sky (US), moves typically start around index 3200-3300
        let move_names = self.extract_move_names_from_strings(&strings)?;

        Ok(move_names)
    }

    /// Parse the text_*.str string table format
    fn parse_string_table(&self, data: &[u8]) -> io::Result<Vec<String>> {
        let mut cursor = Cursor::new(data);
        let mut pointers = Vec::new();

        // Read pointers until we hit a pointer that equals or exceeds the file size
        loop {
            if cursor.position() as usize + 4 > data.len() {
                break;
            }

            let ptr = read_u32_le(&mut cursor)?;

            // If pointer is beyond file size, we've read all pointers
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

        // Extract strings from the pointers
        let mut strings = Vec::with_capacity(pointers.len());

        for i in 0..pointers.len() - 1 {
            let start = pointers[i] as usize;
            let end = pointers[i + 1] as usize;

            if start >= data.len() || end > data.len() || start >= end {
                strings.push(String::new());
                continue;
            }

            // Read null-terminated string
            let string_data = &data[start..end];
            let null_pos = string_data
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(string_data.len());
            let str_bytes = &string_data[..null_pos];

            // Convert to string (handle encoding - typically ISO 8859-1 for English)
            let text = String::from_utf8_lossy(str_bytes).to_string();
            strings.push(text);
        }

        Ok(strings)
    }

    fn extract_move_names_from_strings(&self, strings: &[String]) -> io::Result<Vec<String>> {
        const MOVE_NAMES_BEGIN: usize = 8173; // For EoS NA
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

        // Extract the sequential move names
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

        // Seek to the data pointer (Waza Header)
        cursor.set_position(sir0.data_pointer as u64);

        // Read Waza Header (2 pointers)
        let ptr_moves_data = read_u32_le(&mut cursor)?;
        let _ptr_moveset_table = read_u32_le(&mut cursor)?;

        println!("  Moves data pointer: 0x{:X}", ptr_moves_data);

        // Seek to moves data
        cursor.set_position(ptr_moves_data as u64);

        let mut moves = Vec::new();
        let mut move_index = 0;

        // Read move entries until we hit padding (0xAA bytes)
        loop {
            let current_pos = cursor.position() as usize;

            // Check if we've hit padding or end of data
            if current_pos + 26 > sir0.content.len() {
                break;
            }

            // Check for 0xAA padding marker
            if sir0.content[current_pos] == 0xAA {
                println!("  Found padding marker at offset 0x{:X}", current_pos);
                break;
            }

            // Get move name from the string table
            let name = move_names
                .get(move_index)
                .cloned()
                .unwrap_or_else(|| format!("Unknown_{:04}", move_index));

            // Parse 26-byte move entry
            let move_data = self.parse_move_entry(&mut cursor, name)?;
            moves.push(move_data);

            move_index += 1;
        }

        Ok(moves)
    }

    /// Parse a single 26-byte move entry
    fn parse_move_entry(&self, cursor: &mut Cursor<&[u8]>, name: String) -> io::Result<MoveData> {
        // Offset 0x00: Base Power (u16)
        let base_power = read_u16_le(cursor)?;

        // Offset 0x02: Type (u8)
        let move_type = read_u8(cursor)?;

        // Offset 0x03: Category (u8)
        let category = read_u8(cursor)?;

        // Skip bytes 0x04-0x07 (4 bytes)
        cursor.set_position(cursor.position() + 4);

        // Offset 0x08: Base PP (u8)
        let base_pp = read_u8(cursor)?;

        // Skip bytes 0x09-0x0A (2 bytes)
        cursor.set_position(cursor.position() + 2);

        // Offset 0x0B: Accuracy (u8)
        let accuracy = read_u8(cursor)?;

        // Skip bytes 0x0C-0x15 (10 bytes)
        cursor.set_position(cursor.position() + 10);

        // Offset 0x16: Move ID (u16)
        let move_id = read_u16_le(cursor)?;

        // Skip remaining bytes to complete 26-byte entry
        cursor.set_position(cursor.position() + 2);

        Ok(MoveData {
            move_id,
            name,
            base_power,
            move_type,
            category,
            base_pp,
            accuracy,
        })
    }

    /// Save move lookup JSON (name -> ID mapping) with snake_case keys
    fn save_move_lookup(&self, moves: &[MoveData], output_dir: &Path) -> io::Result<()> {
        fn to_snake_case(s: &str) -> String {
            let mut result = String::with_capacity(s.len() + 3); // pre-allocate for `_`

            for (i, ch) in s.chars().enumerate() {
                match ch {
                    ' ' | '-' => result.push('_'),
                    '\'' => continue, // skip apostrophes
                    // create var `c` if `c.is_uppercase()` is true. For pascal case scenarios
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
            .map(|m| (to_snake_case(&m.name), m.move_id)) // transforms each element into a tuple
            .collect(); // convert the tuple to a HashMap

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

        serde_json::to_writer_pretty(file, moves)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        println!("  Saved move data to {}", output_path.display());
        Ok(())
    }
}
