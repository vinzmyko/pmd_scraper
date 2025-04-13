//! WAN sprite format handling
//!
//! This module provides functionality for parsing Pokémon Mystery Dungeon
//! WAN sprite files, which are used for character animations and battle effects.

use std::{
    fmt,
    io::{self},
};

pub mod model;
pub mod parser;
pub mod renderer;

pub use model::*;
pub use parser::*;

/// Coordinates for sprite positioning
pub const CENTRE_X: i16 = 256;
pub const CENTRE_Y: i16 = 512;

pub const TEX_SIZE: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WanType {
    Character,
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
    pub const _ATTR0_FLAG_BITS_MASK: u16 = 0xFC00; // 1111 1100 0000 0000
    pub const ATTR0_COL_PAL_MASK: u16 = 0x2000; // 0010 0000 0000 0000
    pub const ATTR0_Y_OFFSET_MASK: u16 = 0x03FF; // 0000 0011 1111 1111

    pub const _ATTR1_FLAG_BITS_MASK: u16 = 0xFE00; // 1111 1110 0000 0000
    pub const ATTR1_VFLIP_MASK: u16 = 0x2000; // 0010 0000 0000 0000
    pub const ATTR1_HFLIP_MASK: u16 = 0x1000; // 0001 0000 0000 0000
    pub const ATTR1_IS_LAST_MASK: u16 = 0x0800; // 0000 1000 0000 0000
    pub const ATTR1_X_OFFSET_MASK: u16 = 0x01FF; // 0000 0001 1111 1111

    pub const _ATTR2_FLAG_BITS_MASK: u16 = 0xFC00; // 1111 1100 0000 0000
    pub const ATTR2_PAL_NUMBER_MASK: u16 = 0xF000; // 1111 0000 0000 0000
    pub const ATTR2_TILE_NUM_MASK: u16 = 0x03FF; // 0000 0011 1111 1111

    // ATTR0/1 combined resolution flags
    pub const ATTR01_RES_MASK: u16 = 0xC000; // 1100 0000 0000 0000

    pub const FRAME_HIT_MASK: u8 = 0x02; // 0000 0010
    pub const FRAME_RETURN_MASK: u8 = 0x01; // 0000 0001
}

/// Error type for WAN operations
#[derive(Debug)]
pub enum WanError {
    InvalidDataStructure(String),
    Io(io::Error),
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
            WanError::InvalidDataStructure(msg) => write!(f, "Invalid WAN data: {}", msg),
            WanError::Io(err) => write!(f, "I/O error: {}", err),
            WanError::OutOfBounds(msg) => write!(f, "Out of bounds: {}", msg),
        }
    }
}
