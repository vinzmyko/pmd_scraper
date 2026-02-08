//! # Dungeon Palette List Animation
//!
//! Handles palette cycling used for animating water, lava, crystals etc. Swaps specific colours in
//! the palette over time.

use std::io;

use super::dpl::Rgb;

const DPLA_TOTAL_COLOURS: usize = 32; // 16 for palette 10 + 16 for palette 11

pub struct DplaColourEntry {
    pub num_frames: u16,
    pub duration: u16, // in 1/60s frames
    pub frames: Vec<Rgb>,
}

pub struct Dpla {
    pub colours: Vec<DplaColourEntry>, // always length 32
}

impl Dpla {
    /// Parse from SIR0-unwrapped content + data_pointer
    pub fn from_sir0_content(content: &[u8], data_pointer: u32) -> Result<Self, io::Error> {
        let dp = data_pointer as usize;
        let read_u32 = |off: usize| -> Result<usize, io::Error> {
            if off + 4 > content.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("DPLA read out of bounds at offset {}", off),
                ));
            }
            Ok(u32::from_le_bytes([
                content[off],
                content[off + 1],
                content[off + 2],
                content[off + 3],
            ]) as usize)
        };

        let mut colours = Vec::with_capacity(DPLA_TOTAL_COLOURS);

        for i in 0..DPLA_TOTAL_COLOURS {
            let colour_ptr = read_u32(dp + i * 4)?;

            if colour_ptr + 4 > content.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("DPLA colour entry {} out of bounds at {}", i, colour_ptr),
                ));
            }
            let num_colours = u16::from_le_bytes([content[colour_ptr], content[colour_ptr + 1]]);
            let duration = u16::from_le_bytes([content[colour_ptr + 2], content[colour_ptr + 3]]);

            let frame_count = num_colours.max(1) as usize;
            let mut frames = Vec::with_capacity(frame_count);
            for j in 0..frame_count {
                let base = colour_ptr + 4 + j * 4;
                if base + 3 > content.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("DPLA frame data out of bounds at {}", base),
                    ));
                }
                frames.push(Rgb {
                    r: content[base],
                    g: content[base + 1],
                    b: content[base + 2],
                });
            }
            colours.push(DplaColourEntry {
                num_frames: num_colours,
                duration,
                frames,
            });
        }

        Ok(Dpla { colours })
    }

    /// Whether palette 10 or 11 has any animated colours
    pub fn has_animation_for_palette(&self, pal_idx: usize) -> bool {
        if pal_idx != 10 && pal_idx != 11 {
            return false;
        }
        let base = if pal_idx == 10 { 0 } else { 16 };
        (base..base + 16).any(|i| self.colours[i].num_frames > 0)
    }
}
