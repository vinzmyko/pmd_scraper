mod filesystem;
mod formats;
mod rom;

use formats::narc;

use filesystem::{FileAllocationTable, FileNameTable};
use rom::{read_header, Rom};

fn main() {
    let rom_eu = Rom::new("../../ROMs/pmd_eos_us.nds");
    let header = read_header(&rom_eu.path);
    println!("ROM Header: {:#?}", header);

    let rom_data = std::fs::read(&rom_eu.path).expect("Failed to read ROM file");

    let fat =
        match FileAllocationTable::read_from_rom(&rom_data, header.fat_offset, header.fat_size) {
            Ok(fat) => fat,
            Err(e) => {
                println!("Error reading FAT: {}", e);
                return;
            }
        };

    println!("FAT contains {} entries", fat.entries.len());

    let fnt = match FileNameTable::read_from_rom(&rom_data, header.fnt_offset) {
        Ok(fnt) => fnt,
        Err(e) => {
            println!("Error reading FNT: {}", e);
            return;
        }
    };

    // Print basic FNT stats
    println!("\nFNT contains:");
    println!("- {} directories", fnt.directories.len());
    println!("- {} files", fnt.file_names.len());

    // Print the first few root directory entries
    println!("\nFirst 5 root directory entries:");
    if let Some(root_children) = fnt.directory_structure.get(&0xF000) {
        for (i, &child_id) in root_children.iter().take(5).enumerate() {
            if let Some(name) = fnt.directory_names.get(&child_id) {
                println!("{}. {} (ID: 0x{:X})", i + 1, name, child_id);
            }
        }
    }

    // Try to find a specific file
    let test_file = "MONSTER/monster.bin";
    if let Some(file_id) = fnt.get_file_id(test_file) {
        println!("\nFound file '{}' with ID: {}", test_file, file_id);
        let entry = &fat.entries[file_id as usize];
        println!(
            "  Start: 0x{:X}, End: 0x{:X}, Size: {} bytes",
            entry.start_address,
            entry.end_address,
            entry.size()
        );
    } else {
        println!("\nFile '{}' not found", test_file);
    }
}
