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
    gamecode: String,
    makercode: String,
    unitcode: u8,
    nds_region: u8,
    rom_version: u8
}

fn read_header(rom_path: &PathBuf) -> RomHeader {
    let mut file = File::open(rom_path).unwrap();
    
    // Read game title (0x000, 12 bytes)
    let mut title_buffer = [0u8; 12];
    file.seek(SeekFrom::Start(0x000)).unwrap();
    file.read_exact(&mut title_buffer).unwrap();
    
    // Read gamecode (0x00C, 4 bytes)
    let mut gamecode_buffer = [0u8; 4];
    file.seek(SeekFrom::Start(0x00C)).unwrap();
    file.read_exact(&mut gamecode_buffer).unwrap();
    
    // Read makercode (0x010, 2 bytes)
    let mut makercode_buffer = [0u8; 2];
    file.seek(SeekFrom::Start(0x010)).unwrap();
    file.read_exact(&mut makercode_buffer).unwrap();
    
    // Read single bytes
    let mut unitcode = [0u8; 1];
    file.seek(SeekFrom::Start(0x012)).unwrap();
    file.read_exact(&mut unitcode).unwrap();

    let mut region = [0u8; 1];
    file.seek(SeekFrom::Start(0x01D)).unwrap();
    file.read_exact(&mut region).unwrap();

    let mut version = [0u8; 1];
    file.seek(SeekFrom::Start(0x01E)).unwrap();
    file.read_exact(&mut version).unwrap();

    RomHeader {
        game_title: String::from_utf8_lossy(&title_buffer)
            .trim_end_matches('\0')
            .to_string(),
        gamecode: String::from_utf8_lossy(&gamecode_buffer).to_string(),
        makercode: String::from_utf8_lossy(&makercode_buffer).to_string(),
        unitcode: unitcode[0],
        nds_region: region[0],
        rom_version: version[0],
    }
}

fn main() {
    let rom_eu = Rom::new("../../ROMs/pmd_eos_us.nds");
    println!("{:#?}", read_header(&rom_eu.path));
}
