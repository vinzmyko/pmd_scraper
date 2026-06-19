//! Weather colour-table from dungeon.bin[1034].
//!
//! SIR0-wrapped; after unwrap the colormap block starts at content[0]
//! Layout: 8 maps x 0x400 bytes, each 256 RGBX entries with the 4th byte == 0xFF.

use std::io;

pub const COLVEC_MAP_LEN: usize = 0x400; // 256 colours * 4 bytes
pub const COLVEC_COLORS: usize = 256;
pub const WEATHER_COUNT: usize = 8;

pub struct Colvec {
    /// `colormaps[weather_id]` = 256 RGB triples.
    pub colormaps: Vec<Vec<(u8, u8, u8)>>,
}

impl Colvec {
    /// Parse from SIR0-unwrapped content. The colormap block begins at byte 0.
    pub fn from_sir0_content(content: &[u8]) -> io::Result<Self> {
        let need = WEATHER_COUNT * COLVEC_MAP_LEN;
        if content.len() < need {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("colvec too small: {} < {}", content.len(), need),
            ));
        }

        let mut colormaps = Vec::with_capacity(WEATHER_COUNT);
        for w in 0..WEATHER_COUNT {
            let base = w * COLVEC_MAP_LEN;
            let mut map = Vec::with_capacity(COLVEC_COLORS);
            for c in content[base..base + COLVEC_MAP_LEN].chunks_exact(4) {
                // RGBX fingerprint: guards against the wrong index (1023 is the
                // fade-to-black colvec of identical shape).
                if c[3] != 0xFF {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "colvec RGBX 4th byte != 0xFF (wrong dungeon.bin index?)",
                    ));
                }
                map.push((c[0], c[1], c[2]));
            }
            colormaps.push(map);
        }

        let cv = Colvec { colormaps };
        cv.assert_identity_map0()?;
        Ok(cv)
    }

    /// Map 0 (clear) is an identity ramp (0,0,0)..(255,255,255): a cheap check
    /// that we read the right file at the right offset.
    fn assert_identity_map0(&self) -> io::Result<()> {
        let (r0, _, _) = self.colormaps[0][0];
        let (r255, g255, b255) = self.colormaps[0][255];
        if r0 > 8 || r255 < 247 || g255 < 247 || b255 < 247 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "colvec map 0 is not an identity ramp (unexpected file/offset)",
            ));
        }
        Ok(())
    }
}
