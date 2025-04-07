//! WAN sprite format handling
//!
//! This module provides functionality for parsing Pokémon Mystery Dungeon
//! WAN sprite files, which are used for character animations and battle effects.
//! This implementation is compatible with SkyTemple's WAN parsing.

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use image::{Rgba, RgbaImage};

// Re-exports
pub mod model;
pub mod parser;
pub mod renderer;

pub use model::*;
pub use parser::*;
pub use renderer::*;

/// Center X coordinate for sprite positioning
pub const CENTER_X: i16 = 256;

/// Center Y coordinate for sprite positioning
pub const CENTER_Y: i16 = 512;

/// Used to indicate a frame reference to a previous frame in the same group
pub const MINUS_FRAME: i16 = -1;

/// Size of texture blocks in pixels
pub const TEX_SIZE: usize = 8;

/// Display type for WAN files
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WanType {
    /// Character sprites (monster.bin, m_ground.bin, m_attack.bin)
    Character,
    /// Effect sprites (effect.bin entries 0-267, 290-293)
    Effect,
}

impl fmt::Display for WanType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WanType::Character => write!(f, "Character"),
            WanType::Effect => write!(f, "Effect"),
        }
    }
}

/// Object rendering modes
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjMode {
    Normal = 0,
    SemiTransparent = 1,
    Window = 2,
    Bitmap = 3,
}

/// Texture dimension combinations
/// Each entry represents (width, height) in texture blocks
pub const DIM_TABLE: [(usize, usize); 12] = [
    (1, 1), // 0: 8x8
    (2, 2), // 1: 16x16
    (4, 4), // 2: 32x32
    (8, 8), // 3: 64x64
    (2, 1), // 4: 16x8
    (4, 1), // 5: 32x8
    (4, 2), // 6: 32x16
    (8, 4), // 7: 64x32
    (1, 2), // 8: 8x16
    (1, 4), // 9: 8x32
    (2, 4), // 10: 16x32
    (4, 8), // 11: 32x64
];

// Attribute flag masks
pub mod flags {
    // ATTR0 flags
    pub const ATTR0_FLAG_BITS_MASK: u16 = 0xFC00; // 1111 1100 0000 0000
    pub const ATTR0_COL_PAL_MASK: u16 = 0x2000; // 0010 0000 0000 0000
    pub const ATTR0_MOSAIC_MASK: u16 = 0x1000; // 0001 0000 0000 0000
    pub const ATTR0_OBJMODE_MASK: u16 = 0x0C00; // 0000 1100 0000 0000
    pub const ATTR0_Y_OFFSET_MASK: u16 = 0x03FF; // 0000 0011 1111 1111

    // ATTR1 flags
    pub const ATTR1_FLAG_BITS_MASK: u16 = 0xFE00; // 1111 1110 0000 0000
    pub const ATTR1_VFLIP_MASK: u16 = 0x2000; // 0010 0000 0000 0000
    pub const ATTR1_HFLIP_MASK: u16 = 0x1000; // 0001 0000 0000 0000
    pub const ATTR1_IS_LAST_MASK: u16 = 0x0800; // 0000 1000 0000 0000
    pub const ATTR1_ROTNSCALE_PRM: u16 = 0x3E00; // 0011 1110 0000 0000
    pub const ATTR1_X_OFFSET_MASK: u16 = 0x01FF; // 0000 0001 1111 1111

    // ATTR2 flags
    pub const ATTR2_FLAG_BITS_MASK: u16 = 0xFC00; // 1111 1100 0000 0000
    pub const ATTR2_PAL_NUMBER_MASK: u16 = 0xF000; // 1111 0000 0000 0000
    pub const ATTR2_PRIORITY_MASK: u16 = 0x0C00; // 0000 1100 0000 0000
    pub const ATTR2_TILE_NUM_MASK: u16 = 0x03FF; // 0000 0011 1111 1111

    // ATTR0/1 combined resolution flags
    pub const ATTR01_RES_MASK: u16 = 0xC000; // 1100 0000 0000 0000

    // Frame flags
    pub const FRAME_HIT_MASK: u8 = 0x02; // 0000 0010
    pub const FRAME_RETURN_MASK: u8 = 0x01; // 0000 0001
}

/// Main entry point for parsing WAN files
pub fn parse_wan(data: &[u8], wan_type: WanType) -> Result<WanFile, WanError> {
    // Delegate to parser module
    parser::parse_wan(data, wan_type)
}

/// Extract a single frame from a WAN file
pub fn extract_frame(wan: &WanFile, frame_idx: usize) -> Result<RgbaImage, WanError> {
    // Delegate to renderer module
    renderer::extract_frame(wan, frame_idx)
}

/// Error type for WAN operations
#[derive(Debug)]
pub enum WanError {
    /// Invalid data structure
    InvalidData(String),
    /// I/O error
    Io(io::Error),
    /// SIR0 error
    Sir0Error(String),
    /// Image processing error
    ImageError(String),
    /// Out of bounds access
    OutOfBounds(String),
}

impl From<io::Error> for WanError {
    fn from(err: io::Error) -> Self {
        WanError::Io(err)
    }
}

impl fmt::Display for WanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WanError::InvalidData(msg) => write!(f, "Invalid WAN data: {}", msg),
            WanError::Io(err) => write!(f, "I/O error: {}", err),
            WanError::Sir0Error(msg) => write!(f, "SIR0 error: {}", msg),
            WanError::ImageError(msg) => write!(f, "Image error: {}", msg),
            WanError::OutOfBounds(msg) => write!(f, "Out of bounds: {}", msg),
        }
    }
}

impl std::error::Error for WanError {}
