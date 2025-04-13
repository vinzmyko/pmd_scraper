//! Data structures for WAN sprite files
//!
//! This module defines the core data structures used to represent
//! WAN sprite data

use super::{WanType, CENTRE_X, CENTRE_Y, DIM_TABLE, flags};

#[derive(Debug, Clone)]
pub struct WanFile {
    pub img_data: Vec<ImgPiece>,
    pub frame_data: Vec<MetaFrame>,
    pub animation_groups: Vec<Vec<Animation>>,
    pub body_part_offset_data: Vec<FrameOffset>,
    pub custom_palette: Vec<Vec<(u8, u8, u8, u8)>>,
    pub sdw_size: u8,
    pub wan_type: WanType,
}

/// A collection of image data strips
#[derive(Debug, Clone)]
pub struct ImgPiece {
    /// Pixel data organized as strips
    pub img_px: Vec<Vec<u8>>,
    pub z_sort: u32,
}

/// A collection of meta frame pieces that form a complete sprite frame
#[derive(Debug, Clone)]
pub struct MetaFrame {
    /// Individual pieces of the frame
    pub pieces: Vec<MetaFramePiece>,
}

/// Individual component of a meta frame
#[derive(Debug, Clone)]
pub struct MetaFramePiece {
    /// Index into img_data (-1 means reference a previous piece)
    pub img_index: i16,
    /// Y offset and flags
    pub attr0: u16,
    /// X offset and flags
    pub attr1: u16,
    /// Palette and tile info
    pub attr2: u16,
}

impl MetaFramePiece {
    pub fn new(img_index: i16, attr0: u16, attr1: u16, attr2: u16) -> Self {
        Self {
            img_index,
            attr0,
            attr1,
            attr2,
        }
    }

    /// Check if using 256 colour palette mode
    pub fn is_colour_pal_256(&self) -> bool {
        (self.attr0 & flags::ATTR0_COL_PAL_MASK) != 0
    }

    /// Get Y offset relative to centre
    pub fn get_y_offset(&self) -> i16 {
        let raw_y = self.attr0 & flags::ATTR0_Y_OFFSET_MASK;
        (raw_y as i16) - (CENTRE_Y as i16)
    }

    pub fn is_v_flip(&self) -> bool {
        (self.attr1 & flags::ATTR1_VFLIP_MASK) != 0
    }

    pub fn is_h_flip(&self) -> bool {
        (self.attr1 & flags::ATTR1_HFLIP_MASK) != 0
    }

    /// Get X offset relative to centre
    pub fn get_x_offset(&self) -> i16 {
        let raw_x = self.attr1 & flags::ATTR1_X_OFFSET_MASK;
        (raw_x as i16) - (CENTRE_X as i16)
    }

    /// Get palette number
    pub fn get_pal_num(&self) -> u8 {
        ((self.attr2 & flags::ATTR2_PAL_NUMBER_MASK) >> 12) as u8
    }

    /// Get tile number
    pub fn get_tile_num(&self) -> u16 {
        self.attr2 & flags::ATTR2_TILE_NUM_MASK
    }

    /// Get resolution type index (into DIM_TABLE)
    pub fn get_resolution_type(&self) -> usize {
        ((self.attr1 & flags::ATTR01_RES_MASK) >> 14) as usize |
        ((self.attr0 & flags::ATTR01_RES_MASK) >> 12) as usize
    }

    /// Get the dimensions of this piece
    pub fn get_dimensions(&self) -> (usize, usize) {
        let res_type = self.get_resolution_type();
        if res_type < DIM_TABLE.len() {
            DIM_TABLE[res_type]
        } else {
            // Default to 8x8 if invalid
            (1, 1)
        }
    }

    /// Get the bounds of this piece (x, y, width, height)
    pub fn get_bounds(&self) -> (i16, i16, i16, i16) {
        let start = (self.get_x_offset(), self.get_y_offset());
        let (width, height) = self.get_dimensions();
        
        (
            start.0, 
            start.1, 
            start.0 + (width * super::TEX_SIZE) as i16, 
            start.1 + (height * super::TEX_SIZE) as i16
        )
    }
}

/// Body part offset data for a frame
#[derive(Debug, Clone)]
pub struct FrameOffset {
    /// Head position (x, y)
    pub head: (i16, i16),
    
    /// Left hand position (x, y)
    pub lhand: (i16, i16),
    
    /// Right hand position (x, y)
    pub rhand: (i16, i16),
    
    /// Centre position (x, y)
    pub centre: (i16, i16),
}

impl FrameOffset {
    /// Create a new FrameOffset
    pub fn new(head: (i16, i16), lhand: (i16, i16), rhand: (i16, i16), centre: (i16, i16)) -> Self {
        Self {
            head,
            lhand,
            rhand,
            centre,
        }
    }

    /// Get the bounds that encompass all offsets
    pub fn get_bounds(&self) -> (i16, i16, i16, i16) {
        let mut min_x = i16::MAX;
        let mut min_y = i16::MAX;
        let mut max_x = i16::MIN;
        let mut max_y = i16::MIN;

        for &(x, y) in &[self.head, self.lhand, self.rhand, self.centre] {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }

        (min_x, min_y, max_x + 1, max_y + 1)
    }
}

/// A frame in an animation sequence
#[derive(Debug, Clone)]
pub struct SequenceFrame {
    pub frame_index: u16,
    /// in 1/60ths of a second
    pub duration: u8,
    /// Special flags (bit 0 = return, bit 1 = hit)
    pub flag: u8,
    pub is_rush_point: bool,
    /// Sprite offset from centre (x, y)
    pub offset: (i16, i16),
    /// Shadow offset from centre (x, y)
    pub shadow: (i16, i16),
}

impl SequenceFrame {
    pub fn new(frame_index: u16, duration: u8, flag: u8, offset: (i16, i16), shadow: (i16, i16)) -> Self {
        Self {
            frame_index,
            duration,
            flag,
            is_rush_point: false,
            offset,
            shadow,
        }
    }

    pub fn is_rush_point(&self) -> bool {
        self.is_rush_point
    }

    pub fn is_hit_point(&self) -> bool {
        (self.flag & flags::FRAME_HIT_MASK) != 0
    }

    pub fn is_return_point(&self) -> bool {
        (self.flag & flags::FRAME_RETURN_MASK) != 0
    }
}

/// An animation sequence
#[derive(Debug, Clone)]
pub struct Animation {
    /// Frames in this animation
    pub frames: Vec<SequenceFrame>,
}

impl Animation {
    pub fn new(frames: Vec<SequenceFrame>) -> Self {
        Self { frames }
    }

    pub fn empty() -> Self {
        Self { frames: Vec::new() }
    }
}
