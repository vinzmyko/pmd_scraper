use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

use crate::{
    filesystem::{FileAllocationTable, FileNameTable},
    graphics::portrait::{create_portrait_atlas, AtlasType, KaoFile},
    rom::read_header,
};

pub struct PortraitExtractor {
    rom_path: PathBuf,
    rom_data: Vec<u8>,
}

impl PortraitExtractor {
    pub fn new<P: AsRef<Path>>(rom_path: P) -> io::Result<Self> {
        let rom_path = rom_path.as_ref().to_path_buf();
        let mut rom_file = File::open(&rom_path)?;
        let mut rom_data = Vec::new();
        rom_file.read_to_end(&mut rom_data)?;

        Ok(PortraitExtractor { rom_path, rom_data })
    }

    /// Extract portrait atlases from the ROM
    pub fn extract_portrait_atlases(&self, output_dir: &Path) -> io::Result<()> {
        // Create directories
        fs::create_dir_all(output_dir)?;

        // Get the KAO file data
        let kao_data = self.extract_kao_file()?;

        // Parse the KAO file
        let kao_file = match KaoFile::from_bytes(kao_data) {
            Ok(file) => file,
            Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, e)),
        };

        // Generate both atlas types
        self.generate_atlas(&kao_file, AtlasType::Pokedex, output_dir)?;
        self.generate_atlas(&kao_file, AtlasType::Expressions, output_dir)?;

        Ok(())
    }

    // Helper methods
    fn extract_kao_file(&self) -> io::Result<Vec<u8>> {
        // Read ROM header
        let header = read_header(&self.rom_path);

        // Parse FAT and FNT tables
        let fat =
            FileAllocationTable::read_from_rom(&self.rom_data, header.fat_offset, header.fat_size)?;
        let fnt = FileNameTable::read_from_rom(&self.rom_data, header.fnt_offset)?;

        // Get file ID for the KAO file
        let kao_file_id = fnt
            .get_file_id("FONT/kaomado.kao")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "kao.kao not found"))?;

        // Extract KAO file data and convert to Vec<u8> using to_vec()
        fat.get_file_data(kao_file_id as usize, &self.rom_data)
            .map(|data| data.to_vec()) // Convert &[u8] to Vec<u8>
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Failed to extract kao.kao"))
    }

    fn generate_atlas(
        &self,
        kao_file: &KaoFile,
        atlas_type: AtlasType,
        output_dir: &Path,
    ) -> io::Result<()> {
        let type_name = match atlas_type {
            AtlasType::Pokedex => "pokedex",
            AtlasType::Expressions => "expressions",
        };

        let atlas_path = output_dir.join(format!("{}_atlas.png", type_name));

        println!("Generating {} atlas...", type_name);
        match create_portrait_atlas(kao_file, &atlas_type, &atlas_path) {
            Ok(_) => {
                println!(
                    "Successfully created {} atlas at: {}",
                    type_name,
                    atlas_path.display()
                );
                Ok(())
            }
            Err(e) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to create {} atlas: {}", type_name, e),
            )),
        }
    }
}
