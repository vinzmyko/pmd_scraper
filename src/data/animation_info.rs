use serde::{Deserialize, Serialize};
use std::fmt;

// Constants for data sizes
pub const TRAP_DATA_SIZE: usize = 2;
pub const ITEM_DATA_SIZE: usize = 4;
pub const MOVE_DATA_SIZE: usize = 24;
pub const GENERAL_DATA_SIZE: usize = 28;
pub const SPECIAL_MOVE_DATA_SIZE: usize = 6;
pub const HEADER_SIZE: usize = 20; // 5 * 4 bytes

/// Animation point type for move animations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimPointType {
    Head = 0,
    LeftHand = 1,
    RightHand = 2,
    Center = 3,
    None = 255,
}

impl From<u8> for AnimPointType {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Head,
            1 => Self::LeftHand,
            2 => Self::RightHand,
            3 => Self::Center,
            _ => Self::None,
        }
    }
}

impl fmt::Display for AnimPointType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Head => write!(f, "Head"),
            Self::LeftHand => write!(f, "LeftHand"),
            Self::RightHand => write!(f, "RightHand"),
            Self::Center => write!(f, "Center"),
            Self::None => write!(f, "None"),
        }
    }
}

/// Animation type for general animations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimType {
    Invalid = 0,
    WanFile0 = 1,
    WanFile1 = 2,
    WanOther = 3,
    Wat = 4,
    Screen = 5,
    Wba = 6,
}

impl From<u32> for AnimType {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::WanFile0,
            2 => Self::WanFile1,
            3 => Self::WanOther,
            4 => Self::Wat,
            5 => Self::Screen,
            6 => Self::Wba,
            _ => Self::Invalid,
        }
    }
}

impl fmt::Display for AnimType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => write!(f, "Invalid"),
            Self::WanFile0 => write!(f, "WanFile0"),
            Self::WanFile1 => write!(f, "WanFile1"),
            Self::WanOther => write!(f, "WanOther"),
            Self::Wat => write!(f, "Wat"),
            Self::Screen => write!(f, "Screen"),
            Self::Wba => write!(f, "Wba"),
        }
    }
}

/// Represents a trap animation entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapAnimationInfo {
    pub effect_id: u16,
}

/// Represents an item animation entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemAnimationInfo {
    pub effect_id_1: u16,
    pub effect_id_2: u16,
}

/// Represents the raw data format for a move animation entry (matches binary layout)
#[derive(Debug, Clone)]
pub struct RawMoveAnimationInfo {
    pub effect_id_1: u16,
    pub effect_id_2: u16,
    pub effect_id_3: u16,
    pub effect_id_4: u16,
    pub dir: u8,
    pub flag1: bool,
    pub flag2: bool,
    pub flag3: bool,
    pub flag4: bool,
    pub speed: u32,
    pub animation: u8,
    pub point: AnimPointType,
    pub sfx_id: u16,
    pub special_animation_count: u16,
    pub special_animation_start_index: u16,
}

// Helper methods for RawMoveAnimationInfo
impl RawMoveAnimationInfo {
    pub fn direction(&self) -> u8 {
        self.dir
    }

    pub fn flags(&self) -> (bool, bool, bool, bool) {
        (self.flag1, self.flag2, self.flag3, self.flag4)
    }

    // Get the raw flags value as it would appear in ROM
    pub fn flags_raw(&self) -> u32 {
        let mut result: u32 = self.dir as u32 & 0x7;
        if self.flag1 {
            result |= 0x8;
        }
        if self.flag2 {
            result |= 0x10;
        }
        if self.flag3 {
            result |= 0x20;
        }
        if self.flag4 {
            result |= 0x40;
        }
        result
    }
}

/// Represents the final move animation entry with embedded special animations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveAnimationInfo {
    pub effect_id_1: u16,
    pub effect_id_2: u16,
    pub effect_id_3: u16,
    pub effect_id_4: u16,
    pub dir: u8,
    pub flag1: bool,
    pub flag2: bool,
    pub flag3: bool,
    pub flag4: bool,
    pub speed: u32,
    pub animation: u8,
    pub point: AnimPointType,
    pub sfx_id: u16,
    pub special_animations: Vec<SpecialMoveAnimationInfo>,
}

// Helper methods for MoveAnimationInfo
impl MoveAnimationInfo {
    pub fn direction(&self) -> u8 {
        self.dir
    }

    pub fn flags(&self) -> (bool, bool, bool, bool) {
        (self.flag1, self.flag2, self.flag3, self.flag4)
    }

    // Get the raw flags value as it would appear in ROM
    pub fn flags_raw(&self) -> u32 {
        let mut result: u32 = self.dir as u32 & 0x7;
        if self.flag1 {
            result |= 0x8;
        }
        if self.flag2 {
            result |= 0x10;
        }
        if self.flag3 {
            result |= 0x20;
        }
        if self.flag4 {
            result |= 0x40;
        }
        result
    }

    // Create a MoveAnimationInfo from a RawMoveAnimationInfo and list of special animations
    pub fn from_raw(raw: &RawMoveAnimationInfo, specials: Vec<SpecialMoveAnimationInfo>) -> Self {
        Self {
            effect_id_1: raw.effect_id_1,
            effect_id_2: raw.effect_id_2,
            effect_id_3: raw.effect_id_3,
            effect_id_4: raw.effect_id_4,
            dir: raw.dir,
            flag1: raw.flag1,
            flag2: raw.flag2,
            flag3: raw.flag3,
            flag4: raw.flag4,
            speed: raw.speed,
            animation: raw.animation,
            point: raw.point,
            sfx_id: raw.sfx_id,
            special_animations: specials,
        }
    }
}

/// Represents a general animation entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectAnimationInfo {
    pub anim_type: AnimType,
    pub file_index: u32,
    pub unk1: u32,
    pub animation_index: u32,
    pub sfx_id: i32,
    pub unk3: u32,
    pub unk4: bool,
    pub point: AnimPointType,
    pub is_non_blocking: bool,
    pub loop_flag: bool,
}

/// Represents a special monster move animation entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialMoveAnimationInfo {
    pub pokemon_id: u16,
    pub user_animation_index: u8,
    pub point: AnimPointType,
    pub sfx_id: u16,
}

/// Container for all animation data tables
#[derive(Debug, Clone)]
pub struct AnimData {
    pub trap_table: Vec<TrapAnimationInfo>,
    pub item_table: Vec<ItemAnimationInfo>,
    pub raw_move_table: Vec<RawMoveAnimationInfo>,
    pub general_table: Vec<EffectAnimationInfo>,
    pub special_move_table: Vec<SpecialMoveAnimationInfo>,
}

impl AnimData {
    // Transform raw move data into final format with embedded special animations
    pub fn transform_move_data(&self) -> std::collections::HashMap<usize, MoveAnimationInfo> {
        let mut move_map = std::collections::HashMap::new();

        for (idx, raw_move) in self.raw_move_table.iter().enumerate() {
            let mut special_animations = Vec::new();

            // If there are special animations, include them
            if raw_move.special_animation_count > 0 {
                let start_idx = raw_move.special_animation_start_index as usize;
                let end_idx = start_idx + raw_move.special_animation_count as usize;

                // Ensure we don't go out of bounds
                if end_idx <= self.special_move_table.len() {
                    special_animations = self.special_move_table[start_idx..end_idx].to_vec();
                }
            }

            // Create the final MoveAnimationInfo with embedded special animations
            let move_info = MoveAnimationInfo::from_raw(raw_move, special_animations);
            move_map.insert(idx, move_info);
        }

        move_map
    }
}

/// Region-specific data for animation tables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionData {
    pub start_table: u32,
    pub check_addr: u32,
    pub move_animation_table_overlay: u8,
    pub move_animation_table_offset: u32,
    pub effect_animation_table_overlay: u8,
    pub effect_animation_table_offset: u32,
    pub effect_animation_entry_size: u32,
}

/// Constants for different game regions
pub const NA_REGION_DATA: RegionData = RegionData {
    start_table: 0xAFD0,
    check_addr: 0x3420,
    move_animation_table_overlay: 29,
    move_animation_table_offset: 0x3E064,
    effect_animation_table_overlay: 29,
    effect_animation_table_offset: 0x4152C,
    effect_animation_entry_size: 16,
};

pub const EU_REGION_DATA: RegionData = RegionData {
    start_table: 0xAFE8,
    check_addr: 0x3420,
    move_animation_table_overlay: 29,
    move_animation_table_offset: 0x3E184,
    effect_animation_table_overlay: 29,
    effect_animation_table_offset: 0x41654,
    effect_animation_entry_size: 16,
};

pub const JP_REGION_DATA: RegionData = RegionData {
    start_table: 0xAF18,
    check_addr: 0x3424,
    move_animation_table_overlay: 29,
    move_animation_table_offset: 0x3EE94,
    effect_animation_table_overlay: 29,
    effect_animation_table_offset: 0x41354,
    effect_animation_entry_size: 16,
};

/// Determines the region data based on a game code
pub fn get_region_data(game_code: &str) -> Option<RegionData> {
    if game_code.ends_with('E') {
        // YFYE, YFTE, C2SE
        Some(NA_REGION_DATA)
    } else if game_code.ends_with('P') {
        // YFYP, YFTP, C2SP
        Some(EU_REGION_DATA)
    } else if game_code.ends_with('J') {
        // YFYJ, YFTJ, C2SJ
        Some(JP_REGION_DATA)
    } else {
        None
    }
}

/// Helper functions for reading values in little-endian order
fn read_u8(data: &[u8], offset: usize) -> u8 {
    data[offset]
}

fn read_i8(data: &[u8], offset: usize) -> i8 {
    data[offset] as i8
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    let low = data[offset] as u16;
    let high = data[offset + 1] as u16;
    (high << 8) | low
}

fn read_i16(data: &[u8], offset: usize) -> i16 {
    read_u16(data, offset) as i16
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    let b0 = data[offset] as u32;
    let b1 = data[offset + 1] as u32;
    let b2 = data[offset + 2] as u32;
    let b3 = data[offset + 3] as u32;
    b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
}

fn read_i32(data: &[u8], offset: usize) -> i32 {
    read_u32(data, offset) as i32
}

/// Helper function to write u32 in little endian
pub fn write_u32(data: &mut [u8], value: u32, pos: usize) {
    if pos + 4 <= data.len() {
        data[pos] = (value & 0xFF) as u8;
        data[pos + 1] = ((value >> 8) & 0xFF) as u8;
        data[pos + 2] = ((value >> 16) & 0xFF) as u8;
        data[pos + 3] = ((value >> 24) & 0xFF) as u8;
    }
}

/// Parse animation data from binary blob
pub fn parse_animation_data(data: &[u8]) -> Result<AnimData, String> {
    // Check data length
    if data.len() < HEADER_SIZE {
        return Err(format!("Data too short: {} bytes", data.len()));
    }

    // Read table offsets from header
    let trap_table_ptr = read_u32(data, 0);
    let item_table_ptr = read_u32(data, 4);
    let move_table_ptr = read_u32(data, 8);
    let general_table_ptr = read_u32(data, 12);
    let special_move_table_ptr = read_u32(data, 16);

    // Parse trap table
    let mut trap_table = Vec::new();
    for offset in (trap_table_ptr as usize..item_table_ptr as usize).step_by(TRAP_DATA_SIZE) {
        if offset + TRAP_DATA_SIZE > data.len() {
            break;
        }
        let effect_id = read_u16(data, offset);
        trap_table.push(TrapAnimationInfo { effect_id });
    }

    // Parse item table
    let mut item_table = Vec::new();
    for offset in (item_table_ptr as usize..move_table_ptr as usize).step_by(ITEM_DATA_SIZE) {
        if offset + ITEM_DATA_SIZE > data.len() {
            break;
        }
        let anim1 = read_u16(data, offset);
        let anim2 = read_u16(data, offset + 2);
        item_table.push(ItemAnimationInfo {
            effect_id_1: anim1,
            effect_id_2: anim2,
        });
    }

    // Parse move table - now using RawMoveAnimationInfo
    let mut raw_move_table = Vec::new();
    for offset in (move_table_ptr as usize..general_table_ptr as usize).step_by(MOVE_DATA_SIZE) {
        if offset + MOVE_DATA_SIZE > data.len() {
            break;
        }

        let anim1 = read_u16(data, offset);
        let anim2 = read_u16(data, offset + 2);
        let anim3 = read_u16(data, offset + 4);
        let anim4 = read_u16(data, offset + 6);

        let flags = read_u32(data, offset + 8);
        let dir = (flags & 0x7) as u8;
        let flag1 = (flags & 0x8) != 0;
        let flag2 = (flags & 0x10) != 0;
        let flag3 = (flags & 0x20) != 0;
        let flag4 = (flags & 0x40) != 0;

        let speed = read_u32(data, offset + 12);
        let animation = read_u8(data, offset + 16);
        let point_value = read_u8(data, offset + 17);
        let point = AnimPointType::from(point_value);

        let sfx = read_u16(data, offset + 18);
        let spec_entries = read_u16(data, offset + 20);
        let spec_start = read_u16(data, offset + 22);

        raw_move_table.push(RawMoveAnimationInfo {
            effect_id_1: anim1,
            effect_id_2: anim2,
            effect_id_3: anim3,
            effect_id_4: anim4,
            dir,
            flag1,
            flag2,
            flag3,
            flag4,
            speed,
            animation,
            point,
            sfx_id: sfx,
            special_animation_count: spec_entries,
            special_animation_start_index: spec_start,
        });
    }

    // Parse general animation table
    let mut general_table = Vec::new();
    for offset in
        (general_table_ptr as usize..special_move_table_ptr as usize).step_by(GENERAL_DATA_SIZE)
    {
        if offset + GENERAL_DATA_SIZE > data.len() {
            break;
        }

        let anim_type_value = read_u32(data, offset);
        let anim_type = AnimType::from(anim_type_value);

        let anim_file = read_u32(data, offset + 4);
        let unk1 = read_u32(data, offset + 8);
        let unk2 = read_u32(data, offset + 12);
        let sfx = read_i32(data, offset + 16);
        let unk3 = read_u32(data, offset + 20);
        let unk4 = read_u8(data, offset + 24) != 0;

        let point_value = read_u8(data, offset + 25);
        let point = AnimPointType::from(point_value);

        let unk5 = read_u8(data, offset + 26) != 0;
        let loop_flag = read_u8(data, offset + 27) != 0;

        general_table.push(EffectAnimationInfo {
            anim_type,
            file_index: anim_file,
            unk1,
            animation_index: unk2,
            sfx_id: sfx,
            unk3,
            unk4,
            point,
            is_non_blocking: unk5,
            loop_flag,
        });
    }

    // Parse special move animation table
    let mut special_move_table = Vec::new();
    let data_len = data.len();
    for offset in (special_move_table_ptr as usize..data_len).step_by(SPECIAL_MOVE_DATA_SIZE) {
        if offset + SPECIAL_MOVE_DATA_SIZE > data_len {
            break;
        }

        let pkmn_id = read_u16(data, offset);
        let animation = read_u8(data, offset + 2);

        let point_value = read_u8(data, offset + 3);
        let point = AnimPointType::from(point_value);

        let sfx = read_u16(data, offset + 4);

        special_move_table.push(SpecialMoveAnimationInfo {
            pokemon_id: pkmn_id,
            user_animation_index: animation,
            point,
            sfx_id: sfx,
        });
    }

    Ok(AnimData {
        trap_table,
        item_table,
        raw_move_table,
        general_table,
        special_move_table,
    })
}
