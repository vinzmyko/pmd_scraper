//! # Dungeon Palette List
//!
//! Contain the colour definitions. Defines the RGB values for the 0-15 indices used in the DPCI
//! graphics.

use std::io::{self, Cursor};

use crate::binary_utils::read_u8;

pub const DPL_PAL_COUNT: usize = 12;
pub const DPL_COLOURS_PER_PAL: usize = 16;

#[derive(Clone, Copy, Debug, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub struct Dpl {
    pub palettes: [[Rgb; DPL_COLOURS_PER_PAL]; DPL_PAL_COUNT],
}

impl Dpl {
    pub fn from_bytes(data: &[u8]) -> Result<Self, io::Error> {
        let expected = DPL_PAL_COUNT * DPL_COLOURS_PER_PAL * 4;
        if data.len() < expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("DPL data too short: {} < {}", data.len(), expected),
            ));
        }

        let mut cursor = Cursor::new(data);
        let mut palettes = [[Rgb::default(); DPL_COLOURS_PER_PAL]; DPL_PAL_COUNT];

        for pal in &mut palettes {
            for col in pal {
                *col = Rgb {
                    r: read_u8(&mut cursor)?,
                    g: read_u8(&mut cursor)?,
                    b: read_u8(&mut cursor)?,
                };
                // Skip the 4th byte (always 128/alpha)
                let _ = read_u8(&mut cursor)?;
            }
        }

        Ok(Dpl { palettes })
    }
}
