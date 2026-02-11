//! # Dungeon Graphics Module
//!
//! This module orchestrates the extraction of dungeon tilesets from `dungeon.bin`.
//!
//! # Architecture
//! The graphics data for a single dungeon environment is split across 5 separate
//! file arrays in the ROM, offset by a fixed stride (approx 170 files per type).
//!
//! This module brings them together into a single `DungeonTileset` struct:
//! - DMA: Autotiling rules.
//! - DPC: Chunk composition blueprints.
//! - DPCI: Raw 4bpp tile graphics.
//! - DPL: Colour palettes.
//! - DPLA: Palette animations.

pub mod dma;
pub mod dpc;
pub mod dpci;
pub mod dpl;
pub mod dpla;
pub mod dungeon_names;
pub mod render;

use std::io;

use crate::containers::{
    binpack::BinPack, compression::at4px::At4pxContainer, sir0::Sir0, ContainerHandler,
};

pub struct DungeonTileset {
    pub tileset_id: usize,
    pub dma: dma::Dma,
    pub dpc: dpc::Dpc,
    pub dpci: dpci::Dpci,
    pub dpl: dpl::Dpl,
    pub dpla: dpla::Dpla,
}

pub fn extract_tileset(binpack: &BinPack, tileset_id: usize) -> Result<DungeonTileset, io::Error> {
    // DPLA: SIR0 → parse directly from content
    let dpla_raw = get_file(binpack, tileset_id)?;
    let dpla_sir0 = Sir0::from_bytes(dpla_raw)?;
    let dpla = dpla::Dpla::from_sir0_content(&dpla_sir0.content, dpla_sir0.data_pointer)?;

    // DMA: SIR0 → AT4PX → decompress
    let dma_raw = get_file(binpack, tileset_id + 170)?;
    let dma_sir0 = Sir0::from_bytes(dma_raw)?;
    let dma_at4px = At4pxContainer::deserialise(&dma_sir0.content)?;
    let dma_bytes = dma_at4px
        .decompress()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let dma = dma::Dma::from_bytes(&dma_bytes)?;

    // DPC: AT4PX → decompress
    let dpc_raw = get_file(binpack, tileset_id + 340)?;
    let dpc_at4px = At4pxContainer::deserialise(dpc_raw)?;
    let dpc_bytes = dpc_at4px
        .decompress()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let dpc = dpc::Dpc::from_bytes(&dpc_bytes)?;

    // DPCI: AT4PX → decompress
    let dpci_raw = get_file(binpack, tileset_id + 510)?;
    let dpci_at4px = At4pxContainer::deserialise(dpci_raw)?;
    let dpci_bytes = dpci_at4px
        .decompress()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let dpci = dpci::Dpci::from_bytes(&dpci_bytes)?;

    // DPL: raw bytes, no wrapping
    let dpl_raw = get_file(binpack, tileset_id + 680)?;
    let dpl = dpl::Dpl::from_bytes(dpl_raw)?;

    Ok(DungeonTileset {
        tileset_id,
        dma,
        dpc,
        dpci,
        dpl,
        dpla,
    })
}

fn get_file<'a>(binpack: &'a BinPack, index: usize) -> Result<&'a [u8], io::Error> {
    binpack.get(index).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("BinPack index {} not found", index),
        )
    })
}
