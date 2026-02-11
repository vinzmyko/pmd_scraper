use std::{fs, io, path::Path};

use crate::{
    containers::binpack::BinPack,
    dungeon::{self, render},
    progress::write_progress,
    rom::Rom,
};

const MAX_TILESET_ID: usize = 170;

pub struct DungeonBinExtractor<'a> {
    rom: &'a Rom,
}

impl<'a> DungeonBinExtractor<'a> {
    pub fn new(rom: &'a Rom) -> Self {
        DungeonBinExtractor { rom }
    }

    pub fn extract_dungeon_tilesets(
        &self,
        tileset_ids: Option<Vec<usize>>,
        output_dir: &Path,
        progress_path: &Path,
    ) -> io::Result<()> {
        let dungeon_bin_id = self
            .rom
            .fnt
            .get_file_id("DUNGEON/dungeon.bin")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "dungeon.bin not found"))?;

        let dungeon_bin_data = self
            .rom
            .fat
            .get_file_data(dungeon_bin_id as usize, &self.rom.data)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Failed to extract dungeon.bin")
            })?;

        println!("Parsing dungeon.bin...");
        let binpack = BinPack::from_bytes(dungeon_bin_data)?;
        println!("dungeon.bin contains {} files", binpack.len());

        let ids: Vec<usize> = match tileset_ids {
            Some(ids) => ids.into_iter().filter(|&id| id < MAX_TILESET_ID).collect(),
            None => (0..MAX_TILESET_ID)
                .filter(|id| !(144..170).contains(id))
                .collect(),
        };

        fs::create_dir_all(output_dir)?;
        render::write_layout_json(output_dir)?;

        let mut all_metadata = Vec::new();

        for (i, &tileset_id) in ids.iter().enumerate() {
            println!("Extracting tileset {}...", tileset_id);

            match dungeon::extract_tileset(&binpack, tileset_id) {
                Ok(tileset) => match render::render_tileset(&tileset, output_dir) {
                    Ok(meta) => {
                        let status = if meta.animated { "animated" } else { "static" };
                        println!("  -> {} ({})", meta.filename, status);
                        all_metadata.push(meta);
                    }
                    Err(e) => eprintln!("  -> Error rendering tileset {}: {}", tileset_id, e),
                },
                Err(e) => {
                    eprintln!("  -> Error extracting tileset {}: {}", tileset_id, e);
                }
            }

            write_progress(
                progress_path,
                i + 1,
                ids.len(),
                "dungeon_tileset",
                "running",
            );
        }

        render::write_tilesets_json(&all_metadata, output_dir)?;

        Ok(())
    }
}
