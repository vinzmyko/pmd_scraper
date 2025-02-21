use std::collections::HashMap;
use std::usize;

pub struct FatEntry {
    pub start_address: u32,
    pub end_address: u32,
}

impl FatEntry {
    pub fn size(&self) -> u32 {
        self.end_address - self.start_address
    }
}

pub struct FileAllocationTable {
    pub entries: Vec<FatEntry>,
}

#[allow(dead_code)]
impl FileAllocationTable {
    pub fn read_from_rom(
        rom_data: &[u8],
        fat_offset: u32,
        fat_size: u32,
    ) -> Result<Self, std::io::Error> {
        // Each file entry is 8 bytes long, 4 bytes for each the start and end address
        let num_entries = fat_size / 8;
        let mut entries = Vec::with_capacity(num_entries as usize);

        for i in 0..num_entries {
            // Each entry is 8 bytes long, so update the offset
            let entry_offset = fat_offset as usize + (i as usize * 8);

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
        Some(&rom_data[entry.start_address as usize..entry.end_address as usize])
    }
}

#[allow(dead_code)]
pub struct DirectoryEntry {
    pub offset: u32,        // Offset to sub-table
    pub first_file_id: u16,
    pub parent_id: u16
}

pub enum FntEntry {
    File(String),
    Directory(String, u16)
}

pub struct FileNameTable {
    pub directories: Vec<DirectoryEntry>,
    pub file_names: HashMap<u16, String>,
    pub directory_names: HashMap<u16, String>,
    pub directory_structure: HashMap<u16, Vec<u16>> // Parent ID -> child dir IDs
}

#[allow(dead_code)]
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
    fn read_main_directory_table(&mut self, rom_data: &[u8], fnt_offset: u32) -> Result<(), std::io::Error> {
        // Firstly, read number of dir from root entry
        let total_dirs_offset = fnt_offset as usize + 6;
        let total_dirs = u16::from_le_bytes([   // Converts two bytes to a u16 value which is the
            rom_data[total_dirs_offset],        // number of dirs
            rom_data[total_dirs_offset + 1],
        ]);

        for i in 0..total_dirs {
            // Each sub-table is 8 bytes
            let dir_offset = fnt_offset as usize + (i as usize * 8);

            // Convert 4 bytes starting from dir_offset to hold the subtable_offset
            let subtable_offset = u32::from_le_bytes([
                rom_data[dir_offset],
                rom_data[dir_offset + 1],
                rom_data[dir_offset + 2],
                rom_data[dir_offset + 3]
            ]);

            let first_file_id = u16::from_le_bytes([
                rom_data[dir_offset + 4],
                rom_data[dir_offset + 5]
            ]);

            let parent_or_total = u16::from_le_bytes([
                rom_data[dir_offset + 6],
                rom_data[dir_offset + 7]
            ]);

            // For the root directory (ID 0xF000), this is the total number of directories
            // For other directories, this is the parent directory ID
            let parent_id = if i == 0 { 0xFFFF } else { parent_or_total };

            self.directories.push(DirectoryEntry {
                offset: subtable_offset,
                first_file_id,
                parent_id
            });
        }

        Ok(())
    }

    /// Parse a single sub-table and return its entries
    fn parse_subtable(&self, rom_data: &[u8], fnt_base: u32, subtable_offset: u32) 
    -> Result<Vec<FntEntry>, std::io::Error> {

        let mut entries = Vec::new();
        let mut pos = fnt_base as usize + subtable_offset as usize;

        loop {
            let type_length = rom_data[pos];
            pos += 1;

            // Check for end of table marker
            if type_length == 0 {
                break;
            }

            // Extract the actual length (lower 7 bits)
            let length = type_length & 0x7F;

            // Extract the name (ASCII string)
            let name_bytes = &rom_data[pos..pos + length as usize];
            let name = String::from_utf8_lossy(name_bytes).to_string();
            pos += length as usize;

            // Check if this is a file or directory entry
            if type_length & 0x80 == 0 {
                // File entry (no ID field)
                entries.push(FntEntry::File(name));
            } else {
                // Directory entry (has ID field)
                let dir_id = u16::from_le_bytes([
                    rom_data[pos],
                    rom_data[pos + 1]
                ]);
                pos += 2;
                entries.push(FntEntry::Directory(name, dir_id));
            }
        }

        Ok(entries)
    }

    /// Parse all sub-tables and build our file/directory maps
    fn parse_subtables(&mut self, rom_data: &[u8], fnt_offset: u32) -> Result<(), std::io::Error> {
        // Process each directory's sub-table
        for (dir_index, dir_entry) in self.directories.iter().enumerate() {
            let dir_id = 0xF000 + dir_index as u16;

            // Parse this directory's sub-table
            let entries = self.parse_subtable(rom_data, fnt_offset, dir_entry.offset)?;

            // Track the current file ID 
            let mut file_id = dir_entry.first_file_id;

            // Process each entry
            for entry in entries {
                match entry {
                    FntEntry::File(name) => {
                        // Map this file ID to its name
                        self.file_names.insert(file_id, name);
                        file_id += 1; // File IDs are sequential
                    },
                    FntEntry::Directory(name, child_dir_id) => {
                        // Map this directory ID to its name
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

    /// Get the full path for a file ID
    pub fn get_file_path(&self, file_id: u16) -> Option<String> {
        // Find which directory contains this file ID
        let mut containing_dir_id = None;

        for (i, dir) in self.directories.iter().enumerate() {
            let dir_id = 0xF000 + i as u16;
            let next_dir = if i + 1 < self.directories.len() {
                self.directories[i + 1].first_file_id
            } else {
                u16::MAX
            };

            if file_id >= dir.first_file_id && file_id < next_dir {
                containing_dir_id = Some(dir_id);
                break;
            }
        }

        // If we found the directory, build the path
        if let Some(dir_id) = containing_dir_id {
            if let Some(filename) = self.file_names.get(&file_id) {
                // Get the full path to this directory
                if let Some(dir_path) = self.get_directory_path(dir_id) {
                    if dir_path.is_empty() {
                        return Some(filename.clone());
                    } else {
                        return Some(format!("{}/{}", dir_path, filename));
                    }
                }
            }
        }

        None
    }

    /// Get the full path for a directory ID
    fn get_directory_path(&self, dir_id: u16) -> Option<String> {
        if dir_id == 0xF000 {
            // Root directory has empty path
            return Some(String::new());
        }

        // Find the directory entry
        let dir_index = (dir_id & 0x0FFF) as usize;
        if dir_index >= self.directories.len() {
            return None;
        }

        // Get the directory name
        let dir_name = self.directory_names.get(&dir_id)?;

        // Get the parent directory
        let parent_id = self.directories[dir_index].parent_id;

        // Recursively build the path
        if let Some(parent_path) = self.get_directory_path(parent_id) {
            if parent_path.is_empty() {
                return Some(dir_name.clone());
            } else {
                return Some(format!("{}/{}", parent_path, dir_name));
            }
        }

        None
    }

    /// Get a file ID for a given path
    pub fn get_file_id(&self, path: &str) -> Option<u16> {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.is_empty() {
            return None;
        }

        // Start at the root directory
        let mut current_dir_id = 0xF000;
        let mut i = 0;

        // Traverse directories in the path
        while i < parts.len() - 1 {
            let dir_name = parts[i];

            // Find the child directory with this name
            let mut found = false;
            if let Some(children) = self.directory_structure.get(&current_dir_id) {
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

            i += 1;
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
                // We've reached the end of files in this directory
                break;
            }
        }

        None
    }
}
