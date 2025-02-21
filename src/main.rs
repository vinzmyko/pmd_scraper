use std::path::PathBuf;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

struct Rom {
    path: PathBuf
}

impl Rom {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Rom {
            path: path.into()
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct RomHeader {
    game_title: String,
    game_code: String,
    maker_code: String,
    arm9_rom_offset: u32,
    arm9_entry_address: u32,
    arm9_ram_address: u32,
    arm9_size: u32,
    fnt_offset: u32,
    fnt_size: u32,
    fat_offset: u32,
    fat_size: u32,
    unit_code: u8,
    nds_region: u8,
    rom_version: u8,
    device_capacity: u8,
    encryption_seed: u8,
}

fn read_header(rom_path: &PathBuf) -> RomHeader {
    let mut file = File::open(rom_path).unwrap();
    
    let mut title_buffer = [0u8; 12];
    file.seek(SeekFrom::Start(0x000)).unwrap();
    file.read_exact(&mut title_buffer).unwrap();
    
    let mut game_code_buffer = [0u8; 4];
    file.seek(SeekFrom::Start(0x00C)).unwrap();
    file.read_exact(&mut game_code_buffer).unwrap();
    
    let mut maker_code_buffer = [0u8; 2];
    file.seek(SeekFrom::Start(0x010)).unwrap();
    file.read_exact(&mut maker_code_buffer).unwrap();
    
    let mut unit_code = [0u8; 1];
    file.seek(SeekFrom::Start(0x012)).unwrap();
    file.read_exact(&mut unit_code).unwrap();

    let mut region = [0u8; 1];
    file.seek(SeekFrom::Start(0x01D)).unwrap();
    file.read_exact(&mut region).unwrap();

    let mut version = [0u8; 1];
    file.seek(SeekFrom::Start(0x01E)).unwrap();
    file.read_exact(&mut version).unwrap();

    let mut device_capacity = [0u8; 1];
    file.seek(SeekFrom::Start(0x014)).unwrap();
    file.read_exact(&mut device_capacity).unwrap();

    let mut encrypt_seed = [0u8; 1];
    file.seek(SeekFrom::Start(0x013)).unwrap();
    file.read_exact(&mut encrypt_seed).unwrap();

    let mut arm9_rom_offset = [0u8; 4];
    file.seek(SeekFrom::Start(0x020)).unwrap();
    file.read_exact(&mut arm9_rom_offset).unwrap();

    let mut arm9_entry_addr = [0u8; 4];
    file.seek(SeekFrom::Start(0x024)).unwrap();
    file.read_exact(&mut arm9_entry_addr).unwrap();

    let mut arm9_ram_addr = [0u8; 4];
    file.seek(SeekFrom::Start(0x028)).unwrap();
    file.read_exact(&mut arm9_ram_addr).unwrap();

    let mut arm9_size = [0u8; 4];
    file.seek(SeekFrom::Start(0x02C)).unwrap();
    file.read_exact(&mut arm9_size).unwrap();

    let mut fnt_offset = [0u8; 4];
    file.seek(SeekFrom::Start(0x040)).unwrap();
    file.read_exact(&mut fnt_offset).unwrap();

    let mut fnt_size = [0u8; 4];
    file.seek(SeekFrom::Start(0x044)).unwrap();
    file.read_exact(&mut fnt_size).unwrap();

    let mut fat_offset = [0u8; 4];
    file.seek(SeekFrom::Start(0x048)).unwrap();
    file.read_exact(&mut fat_offset).unwrap();

    let mut fat_size = [0u8; 4];
    file.seek(SeekFrom::Start(0x04C)).unwrap();
    file.read_exact(&mut fat_size).unwrap();

    RomHeader {
        game_title: String::from_utf8_lossy(&title_buffer)
            .trim_end_matches('\0')
            .to_string(),
        game_code: String::from_utf8_lossy(&game_code_buffer).to_string(),
        maker_code: String::from_utf8_lossy(&maker_code_buffer).to_string(),
        unit_code: unit_code[0],
        nds_region: region[0],
        rom_version: version[0],
        device_capacity: device_capacity[0],
        encryption_seed: encrypt_seed[0],
        arm9_rom_offset: u32::from_le_bytes(arm9_rom_offset),
        arm9_entry_address: u32::from_le_bytes(arm9_entry_addr),
        arm9_ram_address: u32::from_le_bytes(arm9_ram_addr),
        arm9_size: u32::from_le_bytes(arm9_size),
        fnt_offset: u32::from_le_bytes(fnt_offset),
        fnt_size: u32::from_le_bytes(fnt_size),
        fat_offset: u32::from_le_bytes(fat_offset),
        fat_size: u32::from_le_bytes(fat_size)
    }
}

fn main() {
    let rom_eu = Rom::new("../../ROMs/pmd_eos_eu.nds");
    println!("{:#?}", read_header(&rom_eu.path));
}
