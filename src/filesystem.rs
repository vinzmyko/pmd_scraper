use std::{collections::HashMap, usize};

// A FatEntry contains the file location
pub struct FatEntry {
    pub start_address: u32, // 4 bytes long
    pub end_address: u32,   // 4 bytes long
}

pub struct FileAllocationTable {
    pub entries: Vec<FatEntry>,
}

impl FileAllocationTable {
    pub fn read_from_rom(
        rom_data: &[u8],
        fat_offset: u32,
        fat_size: u32,
    ) -> Result<Self, std::io::Error> {
        let num_entries = fat_size / 8;

        let mut entries = Vec::with_capacity(num_entries as usize);

        for i in 0..num_entries {
            let entry_offset = fat_offset as usize + (i as usize * 8);

            // Useful for finding if this ROM is corrupted
            if entry_offset + 8 > rom_data.len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "FAT entry offset out of bounds",
                ));
            }

            // Byte conversion - converts 4 consecutive bytes to a u32
            let start = u32::from_le_bytes([
                rom_data[entry_offset],
                rom_data[entry_offset + 1],
                rom_data[entry_offset + 2],
                rom_data[entry_offset + 3],
            ]);

            let end = u32::from_le_bytes([
                rom_data[entry_offset + 4],
                rom_data[entry_offset + 5],
                rom_data[entry_offset + 6],
                rom_data[entry_offset + 7],
            ]);

            // Unused entries have either 0 for start or end
            if start != 0 || end != 0 {
                entries.push(FatEntry {
                    start_address: start,
                    end_address: end,
                });
            }
        }

        Ok(FileAllocationTable { entries })
    }

    pub fn get_file_data<'a>(&self, file_id: usize, rom_data: &'a [u8]) -> Option<&'a [u8]> {
        if file_id >= self.entries.len() {
            return None;
        }

        let entry = &self.entries[file_id];

        if entry.start_address as usize > rom_data.len()
            || entry.end_address as usize > rom_data.len()
        {
            return None;
        }

        Some(&rom_data[entry.start_address as usize..entry.end_address as usize])
    }
}

pub struct DirectoryEntry {
    pub offset: u32, // Offset to sub-table
    pub first_file_id: u16,
    pub _parent_id: u16,
}

pub enum FntEntry {
    File(String),
    Directory(String, u16),
}

/// Base ID for directories in the NDS filesystem
/// Directories have IDs starting from 0xF000, with their index added to this base
const DIRECTORY_ID_BASE: u16 = 0xF000;
const ESTIMATED_ENTRIES_PER_SUBTABLE: usize = 16;
const ESTIMATED_FILES_PER_DIRECTORY: usize = 8;

pub struct FileNameTable {
    pub directories: Vec<DirectoryEntry>,
    pub file_names: HashMap<u16, String>,
    pub directory_names: HashMap<u16, String>,
    pub directory_structure: HashMap<u16, Vec<u16>>, // Parent ID -> child dir IDs
}

impl FileNameTable {
    pub fn read_from_rom(rom_data: &[u8], fnt_offset: u32) -> Result<Self, std::io::Error> {
        let mut fnt = FileNameTable {
            directories: Vec::new(),
            file_names: HashMap::new(),
            directory_names: HashMap::new(),
            directory_structure: HashMap::new(),
        };

        fnt.read_main_directory_table(rom_data, fnt_offset)?;
        fnt.parse_subtables(rom_data, fnt_offset)?;

        Ok(fnt)
    }

    /// Push values to FileNameTable
    fn read_main_directory_table(
        &mut self,
        rom_data: &[u8],
        fnt_offset: u32,
    ) -> Result<(), std::io::Error> {
        if fnt_offset as usize + 8 > rom_data.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "FNT offset out of bounds",
            ));
        }

        // Firstly, read number of dir from root entry
        let total_dirs_offset = fnt_offset as usize + 6;
        let total_dirs = u16::from_le_bytes([
            // Converts two bytes to a u16 value which is the
            rom_data[total_dirs_offset], // number of dirs
            rom_data[total_dirs_offset + 1],
        ]);

        // Pre-allocate the directories vector
        self.directories = Vec::with_capacity(total_dirs as usize);

        for i in 0..total_dirs {
            // Each sub-table is 8 bytes
            let dir_offset = fnt_offset as usize + (i as usize * 8);

            if dir_offset + 8 > rom_data.len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Directory entry offset out of bounds",
                ));
            }

            // Convert 4 bytes starting from dir_offset to hold the subtable_offset
            let subtable_offset = u32::from_le_bytes([
                rom_data[dir_offset],
                rom_data[dir_offset + 1],
                rom_data[dir_offset + 2],
                rom_data[dir_offset + 3],
            ]);

            let first_file_id =
                u16::from_le_bytes([rom_data[dir_offset + 4], rom_data[dir_offset + 5]]);

            let parent_or_total =
                u16::from_le_bytes([rom_data[dir_offset + 6], rom_data[dir_offset + 7]]);

            // For the root directory (ID DIRECTORY_ID_BASE), this is the total number of directories
            // For other directories, this is the parent directory ID
            let parent_id = if i == 0 { 0xFFFF } else { parent_or_total };

            self.directories.push(DirectoryEntry {
                offset: subtable_offset,
                first_file_id,
                _parent_id: parent_id,
            });
        }

        Ok(())
    }

    /// Parse a single sub-table and return its entries
    fn parse_subtable(
        &self,
        rom_data: &[u8],
        fnt_base: u32,
        subtable_offset: u32,
    ) -> Result<Vec<FntEntry>, std::io::Error> {
        let mut entries = Vec::with_capacity(ESTIMATED_ENTRIES_PER_SUBTABLE);
        let mut pos = fnt_base as usize + subtable_offset as usize;

        if pos >= rom_data.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Subtable offset out of bounds",
            ));
        }

        loop {
            if pos >= rom_data.len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Unexpected end of data in subtable",
                ));
            }

            // Highest bit represents file or dir, lower 7 bits represent name length
            let type_and_length_byte = rom_data[pos];
            // Read type_and_length_byte which is one byte
            pos += 1;

            // Check for end of table marker
            if type_and_length_byte == 0 {
                break;
            }

            // `0x7F` = `0b01111111` Nullifies highest bit and stores name length
            let length = type_and_length_byte & 0x7F;

            if pos + length as usize > rom_data.len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Name length exceeds available data",
                ));
            }

            // Extract the name (ASCII string)
            let name_bytes = &rom_data[pos..pos + length as usize];
            let name = String::from_utf8_lossy(name_bytes).to_string();
            pos += length as usize;

            // `0x80` = `0b10000000` Only keeps the highest bit
            if type_and_length_byte & 0x80 == 0 {
                // File entry (no ID field)
                entries.push(FntEntry::File(name));
            } else {
                // Directory entry (has ID field)
                if pos + 2 > rom_data.len() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Directory ID out of bounds",
                    ));
                }

                let dir_id = u16::from_le_bytes([rom_data[pos], rom_data[pos + 1]]);
                pos += 2;
                entries.push(FntEntry::Directory(name, dir_id));
            }
        }

        Ok(entries)
    }

    /// Parse all sub-tables and build our file/directory maps
    fn parse_subtables(&mut self, rom_data: &[u8], fnt_offset: u32) -> Result<(), std::io::Error> {
        let dir_count = self.directories.len();
        self.file_names = HashMap::with_capacity(dir_count * ESTIMATED_FILES_PER_DIRECTORY);
        self.directory_names = HashMap::with_capacity(dir_count);
        self.directory_structure = HashMap::with_capacity(dir_count);

        // Process each directory's sub-table
        for (dir_index, dir_entry) in self.directories.iter().enumerate() {
            let dir_id = DIRECTORY_ID_BASE + dir_index as u16;

            // Get the file entries of this subtable as Vec<FntEntry>
            let entries = self.parse_subtable(rom_data, fnt_offset, dir_entry.offset)?;

            // Track the current file ID
            let mut file_id = dir_entry.first_file_id;

            // Process each entry
            for entry in entries {
                match entry {
                    // Destructure value to be inserted in file_names
                    FntEntry::File(name) => {
                        // Map this file ID to its name
                        self.file_names.insert(file_id, name);
                        file_id += 1; // File IDs are sequential, increment after inserting
                    }
                    FntEntry::Directory(name, child_dir_id) => {
                        self.directory_names.insert(child_dir_id, name);

                        // Add to directory structure (parent -> children relationship)
                        self.directory_structure
                            .entry(dir_id)
                            .or_insert_with(Vec::new)
                            .push(child_dir_id);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get a file ID for a given path
    pub fn get_file_id(&self, path: &str) -> Option<u16> {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.is_empty() {
            return None;
        }

        // Start at the root directory
        let mut current_dir_id = DIRECTORY_ID_BASE;
        let mut dir = 0;

        // Traverse directories in the path
        while dir < parts.len() - 1 {
            let dir_name = parts[dir];

            // Find the child directory with this name
            let mut found = false;
            // Looks up children of current directory
            if let Some(children) = self.directory_structure.get(&current_dir_id) {
                // Goes through each child directory id
                for &child_id in children {
                    if let Some(name) = self.directory_names.get(&child_id) {
                        if name == dir_name {
                            current_dir_id = child_id;
                            found = true;
                            break;
                        }
                    }
                }
            }

            if !found {
                return None;
            }

            dir += 1;
        }

        // Find the file in the current directory
        let dir_index = (current_dir_id & 0x0FFF) as usize;
        if dir_index >= self.directories.len() {
            return None;
        }

        let dir_entry = &self.directories[dir_index];
        let file_name = parts[parts.len() - 1];

        // Find the file ID by searching through files in this directory
        let base_id = dir_entry.first_file_id;
        for id in base_id.. {
            if let Some(name) = self.file_names.get(&id) {
                if name == file_name {
                    return Some(id);
                }
            } else {
                // End of files in current dir
                break;
            }
        }

        None
    }
}
