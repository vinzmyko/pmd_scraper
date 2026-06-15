//! Screen effect format (`effect_animation_info` anim_type 5).
//!
//! These are the non-WAN entries in `/EFFECT/effect.bin`
//! At runtime the ROM renders them as a full-screen BG overlay
//!
//! Frames are laid out tile-by-tile, `COLS` (33) textures wide, into a 256x160 canvas.

use std::fmt;
use std::io::{Cursor, Seek, SeekFrom};

use image::{Rgba, RgbaImage};

use crate::binary_utils::{read_u16_le, read_u32_le, read_u8};

const TEX_SIZE: usize = 8;
const SCREEN_WIDTH: u32 = 256;
const SCREEN_HEIGHT: u32 = 160;
/// Textures per row. The per-frame header field `unk6`
/// is always this value for a valid screen effect.
const COLS: usize = 33;

const ATTR_DRAW: u16 = 0x8000;
const ATTR_FLIP_Y: u16 = 0x0800;
const ATTR_FLIP_X: u16 = 0x0400;
const ATTR_VALUE: u16 = 0x03FF;

/// One draw/skip instruction within a frame.
pub struct ScreenPiece {
    /// Dual-purpose: when `skip` is false this is the texture
    /// index to draw. When `skip` is true it is the number of tiles to advance.
    pub index: u16,
    pub flip_x: bool,
    pub flip_y: bool,
    pub skip: bool,
}

/// A single frame of a screen effect.
pub struct ScreenFrame {
    /// Frame duration in raw game ticks (1/60s). Converted to seconds at export.
    pub duration: u16,
    pub alpha: u16,
    /// Textures per column for this frame (rows).
    pub row_height: u16,
    pub pieces: Vec<ScreenPiece>,
}

/// A fully-parsed screen effect file.
pub struct ScreenEffectFile {
    /// Flat nibble stream. Each value is a palette index (0 = transparent).
    pub img_data: Vec<u8>,
    /// Parsed palettes. The renderer uses `palettes[0]`. The
    /// `effect_animation_info.palette_index` selects palettes only on the runtime
    /// BG path, which this export does not reproduce.
    pub palettes: Vec<Vec<(u8, u8, u8, u8)>>,
    pub frames: Vec<ScreenFrame>,
}

#[derive(Debug)]
pub enum ScreenEffectError {
    Io(std::io::Error),
    Invalid(String),
}

impl fmt::Display for ScreenEffectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScreenEffectError::Io(e) => write!(f, "I/O error: {}", e),
            ScreenEffectError::Invalid(m) => write!(f, "Invalid screen effect: {}", m),
        }
    }
}

impl From<std::io::Error> for ScreenEffectError {
    fn from(e: std::io::Error) -> Self {
        ScreenEffectError::Io(e)
    }
}

impl From<ScreenEffectError> for std::io::Error {
    fn from(e: ScreenEffectError) -> Self {
        match e {
            ScreenEffectError::Io(io_err) => io_err,
            ScreenEffectError::Invalid(m) => {
                std::io::Error::new(std::io::ErrorKind::InvalidData, m)
            }
        }
    }
}

/// Parse a screen effect from SIR0 `content` and its `data_pointer`
pub fn parse_screen_effect(
    content: &[u8],
    data_pointer: u32,
) -> Result<ScreenEffectFile, ScreenEffectError> {
    let mut c = Cursor::new(content);
    let ptr_effect = data_pointer as u64;

    // --- Content header (32 bytes) ---
    c.seek(SeekFrom::Start(ptr_effect))?;
    let nb_frames = read_u32_le(&mut c)?;
    let ptr_anim_data = read_u32_le(&mut c)? as u64;
    let _unk3 = read_u32_le(&mut c)?;
    let ptr_img_data = read_u32_le(&mut c)? as u64;
    let ptr_palette_block = read_u32_le(&mut c)? as u64;
    let _unk1 = read_u16_le(&mut c)?;
    let _unk2 = read_u16_le(&mut c)?;

    // --- Palettes ---
    // Channel read order (byte0, byte1, byte2)
    let mut palettes = Vec::new();
    if ptr_img_data > ptr_palette_block {
        let total_colors = ((ptr_img_data - ptr_palette_block) / 4) as usize;
        let total_palettes = total_colors / 16;
        c.seek(SeekFrom::Start(ptr_palette_block))?;
        for _ in 0..total_palettes {
            let mut pal = Vec::with_capacity(16);
            for _ in 0..16 {
                let c0 = read_u8(&mut c)?;
                let c1 = read_u8(&mut c)?;
                let c2 = read_u8(&mut c)?;
                let _ = read_u8(&mut c)?; // padding
                pal.push((c0, c1, c2, 255));
            }
            palettes.push(pal);
        }
    }
    if palettes.is_empty() {
        palettes.push(vec![(0, 0, 0, 0); 16]);
    }

    // --- Image data: nibble stream (low then high) up to the content header ---
    let mut img_data = Vec::new();
    c.seek(SeekFrom::Start(ptr_img_data))?;
    while c.position() < ptr_effect {
        let px = read_u8(&mut c)?;
        img_data.push(px & 0x0F);
        img_data.push(px >> 4);
    }

    // --- Frame pointer table ---
    c.seek(SeekFrom::Start(ptr_anim_data))?;
    let mut ptr_frames = Vec::with_capacity(nb_frames as usize);
    for _ in 0..nb_frames {
        ptr_frames.push(read_u32_le(&mut c)? as u64);
    }

    // --- Frames: 36-byte header + 2-byte draw/skip instructions ---
    let mut frames = Vec::with_capacity(ptr_frames.len());
    for (idx, &ptr_frame) in ptr_frames.iter().enumerate() {
        c.seek(SeekFrom::Start(ptr_frame))?;
        let _unk5 = read_u16_le(&mut c)?;
        let _unk7 = read_u16_le(&mut c)?;
        let cols_marker = read_u16_le(&mut c)?; // always 0x21 (== COLS)
        let row_height = read_u16_le(&mut c)?;
        let frame_dur = read_u16_le(&mut c)?;
        c.seek(SeekFrom::Current(18))?;
        let alpha = read_u16_le(&mut c)?;
        c.seek(SeekFrom::Current(3))?;
        let _unk4 = read_u8(&mut c)?;
        c.seek(SeekFrom::Current(2))?;

        // Cheap integrity check: a mismatch here almost always means the SIR0
        // pointer math / offsets are misaligned rather than a bad file.
        if cols_marker as usize != COLS {
            eprintln!(
                "  - Warning: screen frame {} cols marker = {:#x} (expected {:#x}); \
                 offsets may be misaligned",
                idx, cols_marker, COLS
            );
        }

        let mut pieces = Vec::new();
        let mut total_slots: u32 = 0;
        let limit = row_height as u32 * COLS as u32;
        loop {
            let dv = read_u16_le(&mut c)?;
            let skip = (ATTR_DRAW & dv) == 0;
            let value = ATTR_VALUE & dv;
            pieces.push(ScreenPiece {
                index: value,
                flip_x: (ATTR_FLIP_X & dv) != 0,
                flip_y: (ATTR_FLIP_Y & dv) != 0,
                skip,
            });
            total_slots += if skip { value as u32 } else { 1 };
            if total_slots >= limit {
                break;
            }
        }

        // End-pointer reconciliation.
        let end_ptr = if idx + 1 < ptr_frames.len() {
            ptr_frames[idx + 1]
        } else {
            ptr_anim_data
        };
        let cur = c.position();
        if cur != end_ptr && cur != end_ptr.wrapping_sub(2) {
            return Err(ScreenEffectError::Invalid(format!(
                "frame {} ended at {:#x}, expected {:#x} or {:#x}",
                idx,
                cur,
                end_ptr,
                end_ptr.wrapping_sub(2)
            )));
        }

        frames.push(ScreenFrame {
            duration: frame_dur,
            alpha,
            row_height,
            pieces,
        });
    }

    Ok(ScreenEffectFile {
        img_data,
        palettes,
        frames,
    })
}

/// Render one frame onto a 256x160 RGBA canvas.
///
/// Per-pixel transparency is baked, identically to the WAN renderer. 
/// The per-frame global `alpha` is deliberately not baked. It is
/// emitted as metadata for the client.
pub fn render_screen_frame(file: &ScreenEffectFile, frame: &ScreenFrame) -> RgbaImage {
    let mut img = RgbaImage::new(SCREEN_WIDTH, SCREEN_HEIGHT);
    let palette = &file.palettes[0];
    let mut cursor = 0usize;

    for piece in &frame.pieces {
        if piece.skip {
            cursor += piece.index as usize;
            continue;
        }
        let mut tex = render_piece(file, palette, piece.index as usize);
        if piece.flip_x {
            tex = image::imageops::flip_horizontal(&tex);
        }
        if piece.flip_y {
            tex = image::imageops::flip_vertical(&tex);
        }
        let bx = (cursor % COLS) as i64 * TEX_SIZE as i64;
        let by = (cursor / COLS) as i64 * TEX_SIZE as i64;
        // The 33rd column starts at x=256 and clips off-canvas, same as the ROM.
        image::imageops::overlay(&mut img, &tex, bx, by);
        cursor += 1;
    }
    img
}

/// Render a single 8x8 texture from the nibble stream.
fn render_piece(file: &ScreenEffectFile, palette: &[(u8, u8, u8, u8)], index: usize) -> RgbaImage {
    let mut t = RgbaImage::new(TEX_SIZE as u32, TEX_SIZE as u32);
    let base = index * TEX_SIZE * TEX_SIZE;
    for py in 0..TEX_SIZE {
        for px in 0..TEX_SIZE {
            // Bounds guard: the only intentional deviation from SkyTemple, which
            // would panic on an out-of-range texture index.
            let pe = *file.img_data.get(base + py * TEX_SIZE + px).unwrap_or(&0) as usize;
            if pe == 0 {
                continue; // index 0 is transparent
            }
            if let Some(&col) = palette.get(pe) {
                t.put_pixel(px as u32, py as u32, Rgba([col.0, col.1, col.2, 255]));
            }
        }
    }
    t
}
