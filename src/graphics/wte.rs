//! # WTE texture
//!
//! The 3D weather/mist overlay textures in dungeon.bin: fog (1001), sandstorm
//! (1005), mist (1031), poison-mist (1003). All four are 16-colour 4bpp, 128x128.

use std::io;

use image::{Rgba, RgbaImage};

const MAGIC: &[u8; 4] = b"WTE\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WteImageType {
    None,      // 0x00: palette only
    Color2bpp, // 0x02
    Color4bpp, // 0x03
    Color8bpp, // 0x04
}

impl WteImageType {
    fn from_u8(v: u8) -> io::Result<Self> {
        match v {
            0x00 => Ok(Self::None),
            0x02 => Ok(Self::Color2bpp),
            0x03 => Ok(Self::Color4bpp),
            0x04 => Ok(Self::Color8bpp),
            // A3I5 (1) / A5I3 (6) land here. None of our four files use them;
            // if this ever fires, a manual NDS alpha-texture decode is needed.
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Unsupported WTE image_type {:#x} (alpha-interpolated NDS format)",
                    other
                ),
            )),
        }
    }

    fn bpp(self) -> usize {
        match self {
            Self::None => 0,
            Self::Color2bpp => 2,
            Self::Color4bpp => 4,
            Self::Color8bpp => 8,
        }
    }

    fn has_image(self) -> bool {
        self.bpp() > 0
    }
}

pub struct Wte {
    pub image_type: WteImageType,
    pub width: u16,
    pub height: u16,
    actual_dim: u8,
    image_data: Vec<u8>,
    /// RGB triples; the palette's 4th byte (0x80) is stripped.
    palette: Vec<(u8, u8, u8)>,
}

impl Wte {
    /// Parse from SIR0-unwrapped `content`, WTE header at `header_pnt`
    /// (the SIR0 data_pointer).
    pub fn from_sir0_content(content: &[u8], header_pnt: u32) -> io::Result<Self> {
        let h = header_pnt as usize;
        if content.len() < h + 0x24 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "WTE header out of bounds",
            ));
        }
        if &content[h..h + 4] != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Missing WTE magic",
            ));
        }

        let rd_u32 = |o: usize| {
            u32::from_le_bytes([content[o], content[o + 1], content[o + 2], content[o + 3]])
        };
        let rd_u16 = |o: usize| u16::from_le_bytes([content[o], content[o + 1]]);

        let pointer_image = rd_u32(h + 0x04) as usize;
        let image_length = rd_u32(h + 0x08) as usize;
        let actual_dim = content[h + 0x0C];
        let image_type = WteImageType::from_u8(content[h + 0x0D])?;
        let width = rd_u16(h + 0x14);
        let height = rd_u16(h + 0x16);
        let pointer_pal = rd_u32(h + 0x18) as usize;
        let number_pal_colors = rd_u32(h + 0x1C) as usize;

        let image_data = if image_type.has_image() {
            let end = pointer_image
                .checked_add(image_length)
                .filter(|&e| e <= content.len())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "WTE image data out of bounds")
                })?;
            content[pointer_image..end].to_vec()
        } else {
            Vec::new()
        };

        let pal_end = number_pal_colors
            .checked_mul(4)
            .and_then(|n| pointer_pal.checked_add(n))
            .filter(|&e| e <= content.len())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "WTE palette out of bounds")
            })?;
        let palette = content[pointer_pal..pal_end]
            .chunks_exact(4)
            .map(|c| (c[0], c[1], c[2]))
            .collect();

        Ok(Wte {
            image_type,
            width,
            height,
            actual_dim,
            image_data,
            palette,
        })
    }

    /// Decoded buffer dimensions implied by the image mode (for our files == width/height).
    pub fn actual_dimensions(&self) -> (usize, usize) {
        let w = 8usize << (self.actual_dim & 0x07);
        let h = 8usize << ((self.actual_dim >> 3) & 0x07);
        (w, h)
    }

    /// Decode to RGBA at the mode's actual dimensions.
    ///
    /// Palette index 0 is emitted transparent; the ~25% overlay alpha is a render
    /// constant applied by the client, deliberately NOT baked here. (If these
    /// textures turn out to want an opaque index 0, this is a one-line flip.)
    pub fn to_rgba(&self) -> io::Result<RgbaImage> {
        if !self.image_type.has_image() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "WTE has no image data",
            ));
        }
        let (w, h) = self.actual_dimensions();
        let bpp = self.image_type.bpp();
        let pixels_per_byte = 8 / bpp;
        let nb_colors = 1usize << bpp;
        let total = w * h;

        let mut img = RgbaImage::new(w as u32, h as u32);
        for (i, &px) in self.image_data.iter().enumerate() {
            for j in 0..pixels_per_byte {
                let lin = i * pixels_per_byte + j;
                if lin >= total {
                    break;
                }
                let idx = ((px >> (bpp * j)) as usize) % nb_colors;
                let rgba = if idx == 0 {
                    Rgba([0, 0, 0, 0])
                } else {
                    match self.palette.get(idx) {
                        Some(&(r, g, b)) => Rgba([r, g, b, 255]),
                        None => Rgba([0, 0, 0, 0]),
                    }
                };
                img.put_pixel((lin % w) as u32, (lin / w) as u32, rgba);
            }
        }
        Ok(img)
    }
}
