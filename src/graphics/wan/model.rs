//! Data structures for WAN sprite files
//!
//! This module defines the core data structures used to represent
//! WAN sprite data, closely aligning with SkyTemple's implementation.

use super::{WanType, CENTER_X, CENTER_Y, DIM_TABLE, MINUS_FRAME, flags};
use image::{RgbaImage, Rgba};

/// Main container for WAN file data
#[derive(Debug, Clone)]
pub struct WanFile {
    /// Image data for all frames
    pub img_data: Vec<ImgPiece>,
    
    /// Meta frame definitions
    pub frame_data: Vec<MetaFrame>,
    
    /// Animation groups (indexed by animation type)
    pub animation_groups: Vec<Vec<Animation>>,
    
    /// Body part offset data for frames
    pub offset_data: Vec<FrameOffset>,
    
    /// Custom palettes for rendering
    pub custom_palette: Vec<Vec<(u8, u8, u8, u8)>>,
    
    /// Shadow size (0=small, 1=medium, 2=large)
    pub sdw_size: u8,
    
    /// Type of WAN file (Character or Effect)
    pub wan_type: WanType,
}

/// A collection of image data strips
#[derive(Debug, Clone)]
pub struct ImgPiece {
    /// Pixel data organized as strips
    pub img_px: Vec<Vec<u8>>,
    
    /// Z-order sorting value
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
    
    /// Attribute 0: Y offset and flags
    pub attr0: u16,
    
    /// Attribute 1: X offset and flags
    pub attr1: u16,
    
    /// Attribute 2: Palette and tile info
    pub attr2: u16,
}

impl MetaFramePiece {
    /// Create a new MetaFramePiece with default attributes
    pub fn new(img_index: i16, attr0: u16, attr1: u16, attr2: u16) -> Self {
        Self {
            img_index,
            attr0,
            attr1,
            attr2,
        }
    }

    /// Clone this piece with the same attributes
    pub fn clone(&self) -> Self {
        MetaFramePiece {
            img_index: self.img_index,
            attr0: self.attr0,
            attr1: self.attr1,
            attr2: self.attr2,
        }
    }

    // Attribute getters and setters

    /// Check if using 256 color palette mode
    pub fn is_color_pal_256(&self) -> bool {
        (self.attr0 & flags::ATTR0_COL_PAL_MASK) != 0
    }

    /// Check if mosaic effect is enabled
    pub fn is_mosaic_on(&self) -> bool {
        (self.attr0 & flags::ATTR0_MOSAIC_MASK) != 0
    }

    /// Get Y offset relative to center
    pub fn get_y_offset(&self) -> i16 {
        let raw_y = self.attr0 & flags::ATTR0_Y_OFFSET_MASK;
        (raw_y as i16) - (CENTER_Y as i16)
    }

    /// Set Y offset relative to center
    pub fn set_y_offset(&mut self, y_val: i16) {
        let raw_y = (y_val + (CENTER_Y as i16)) as u16;
        self.attr0 = (self.attr0 & flags::ATTR0_FLAG_BITS_MASK) | (flags::ATTR0_Y_OFFSET_MASK & raw_y);
    }

    /// Check if vertically flipped
    pub fn is_v_flip(&self) -> bool {
        (self.attr1 & flags::ATTR1_VFLIP_MASK) != 0
    }

    /// Set vertical flip state
    pub fn set_v_flip(&mut self, flip: bool) {
        if flip {
            self.attr1 |= flags::ATTR1_VFLIP_MASK;
        } else {
            self.attr1 &= !flags::ATTR1_VFLIP_MASK;
        }
    }

    /// Check if horizontally flipped
    pub fn is_h_flip(&self) -> bool {
        (self.attr1 & flags::ATTR1_HFLIP_MASK) != 0
    }

    /// Set horizontal flip state
    pub fn set_h_flip(&mut self, flip: bool) {
        if flip {
            self.attr1 |= flags::ATTR1_HFLIP_MASK;
        } else {
            self.attr1 &= !flags::ATTR1_HFLIP_MASK;
        }
    }

    /// Check if this is the last piece in a frame
    pub fn is_last(&self) -> bool {
        (self.attr1 & flags::ATTR1_IS_LAST_MASK) != 0
    }

    /// Set last piece flag
    pub fn set_is_last(&mut self, last: bool) {
        if last {
            self.attr1 |= flags::ATTR1_IS_LAST_MASK;
        } else {
            self.attr1 &= !flags::ATTR1_IS_LAST_MASK;
        }
    }

    /// Get X offset relative to center
    pub fn get_x_offset(&self) -> i16 {
        let raw_x = self.attr1 & flags::ATTR1_X_OFFSET_MASK;
        (raw_x as i16) - (CENTER_X as i16)
    }

    /// Set X offset relative to center
    pub fn set_x_offset(&mut self, x_val: i16) {
        let raw_x = (x_val + (CENTER_X as i16)) as u16;
        self.attr1 = (self.attr1 & flags::ATTR1_FLAG_BITS_MASK) | (flags::ATTR1_X_OFFSET_MASK & raw_x);
    }

    /// Get palette number
    pub fn get_pal_num(&self) -> u8 {
        ((self.attr2 & flags::ATTR2_PAL_NUMBER_MASK) >> 12) as u8
    }

    /// Get render priority
    pub fn get_priority(&self) -> u8 {
        ((self.attr2 & flags::ATTR2_PRIORITY_MASK) >> 10) as u8
    }

    /// Set render priority
    pub fn set_priority(&mut self, priority: u8) {
        self.attr2 = (self.attr2 & !flags::ATTR2_PRIORITY_MASK) | 
                    ((priority as u16) << 10 & flags::ATTR2_PRIORITY_MASK);
    }

    /// Get tile number
    pub fn get_tile_num(&self) -> u16 {
        self.attr2 & flags::ATTR2_TILE_NUM_MASK
    }

    /// Set tile number
    pub fn set_tile_num(&mut self, tile_num: u16) {
        self.attr2 = (self.attr2 & !flags::ATTR2_TILE_NUM_MASK) | 
                    (flags::ATTR2_TILE_NUM_MASK & tile_num);
    }

    /// Get resolution type index (into DIM_TABLE)
    pub fn get_resolution_type(&self) -> usize {
        ((self.attr1 & flags::ATTR01_RES_MASK) >> 14) as usize |
        ((self.attr0 & flags::ATTR01_RES_MASK) >> 12) as usize
    }

    /// Set resolution type (index into DIM_TABLE)
    pub fn set_resolution_type(&mut self, res: usize) {
        self.attr1 = (self.attr1 & !flags::ATTR01_RES_MASK) | (((res as u16) << 14) & flags::ATTR01_RES_MASK);
        self.attr0 = (self.attr0 & !flags::ATTR01_RES_MASK) | (((res as u16) << 12) & flags::ATTR01_RES_MASK);
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
    
    /// Center position (x, y)
    pub center: (i16, i16),
}

impl FrameOffset {
    /// Create a new FrameOffset
    pub fn new(head: (i16, i16), lhand: (i16, i16), rhand: (i16, i16), center: (i16, i16)) -> Self {
        Self {
            head,
            lhand,
            rhand,
            center,
        }
    }

    /// Adjust all offsets by adding the given position
    pub fn add_loc(&mut self, loc: (i16, i16)) {
        self.head = (self.head.0 + loc.0, self.head.1 + loc.1);
        self.lhand = (self.lhand.0 + loc.0, self.lhand.1 + loc.1);
        self.rhand = (self.rhand.0 + loc.0, self.rhand.1 + loc.1);
        self.center = (self.center.0 + loc.0, self.center.1 + loc.1);
    }

    /// Get the bounds that encompass all offsets
    pub fn get_bounds(&self) -> (i16, i16, i16, i16) {
        let mut min_x = i16::MAX;
        let mut min_y = i16::MAX;
        let mut max_x = i16::MIN;
        let mut max_y = i16::MIN;

        for &(x, y) in &[self.head, self.lhand, self.rhand, self.center] {
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
    /// Index into frame_data
    pub frame_index: u16,
    
    /// Frame duration (in 1/60ths of a second)
    pub duration: u8,
    
    /// Special flags (bit 0 = return, bit 1 = hit)
    pub flag: u8,
    
    /// Whether this is a rush point
    pub is_rush_point: bool,
    
    /// Sprite offset from center (x, y)
    pub offset: (i16, i16),
    
    /// Shadow offset from center (x, y)
    pub shadow: (i16, i16),
}

impl SequenceFrame {
    /// Create a new SequenceFrame
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

    /// Check if this is a rush point
    pub fn is_rush_point(&self) -> bool {
        self.is_rush_point
    }

    /// Check if this is a hit point
    pub fn is_hit_point(&self) -> bool {
        (self.flag & flags::FRAME_HIT_MASK) != 0
    }

    /// Check if this is a return point
    pub fn is_return_point(&self) -> bool {
        (self.flag & flags::FRAME_RETURN_MASK) != 0
    }

    /// Set rush point flag
    pub fn set_rush_point(&mut self, rush: bool) {
        self.is_rush_point = rush;
    }

    /// Set hit point flag
    pub fn set_hit_point(&mut self, hit: bool) {
        if hit {
            self.flag |= flags::FRAME_HIT_MASK;
        } else {
            self.flag &= !flags::FRAME_HIT_MASK;
        }
    }

    /// Set return point flag
    pub fn set_return_point(&mut self, ret: bool) {
        if ret {
            self.flag |= flags::FRAME_RETURN_MASK;
        } else {
            self.flag &= !flags::FRAME_RETURN_MASK;
        }
    }
}

/// An animation sequence
#[derive(Debug, Clone)]
pub struct Animation {
    /// Frames in this animation
    pub frames: Vec<SequenceFrame>,
}

impl Animation {
    /// Create a new Animation
    pub fn new(frames: Vec<SequenceFrame>) -> Self {
        Self { frames }
    }

    /// Create an empty Animation
    pub fn empty() -> Self {
        Self { frames: Vec::new() }
    }
}
