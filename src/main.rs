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

    pub fn read_game_title(&self) -> String {
        let mut file = File::open(&self.path).unwrap();
        let mut buffer = [0u8; 12];
        
        file.seek(SeekFrom::Start(0x000)).unwrap();
        file.read_exact(&mut buffer).unwrap();
        
        String::from_utf8_lossy(&buffer)
            .trim_end_matches('\0')
            .to_string()
    }
}

fn main() {
    let rom_eu = Rom::new("../../ROMs/pmd_eos_eu.nds");
    println!("Game Title: {}", rom_eu.read_game_title());
}
