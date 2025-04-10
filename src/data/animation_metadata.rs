use std::collections::HashMap;

// Use Anim_ID's as the m_attack.bin's Anim_ID's for 2, 3, 4 differ between names for different
// pokemon
pub const MONSTER_BIN_ANIMS: &[u8] = &[0, 5, 6, 7, 11];
pub const M_ATTACK_BIN_ANIMS: &[u8] = &[1, 2, 3, 4, 8, 9, 10, 11, 12];

#[allow(dead_code)]
pub struct AnimationInfo {
    pub id: u8,                 // Animation ID (0-12)
    pub name: &'static str,     // Friendly name (for directories/debugging)
    pub source: &'static str,   // Source bin file ("monster" or "m_attack")
    pub max_frames: u8,         // Expected max frames
    pub group_index: u8,        // Index in the animation group array
    pub single_direction: bool, // Whether this animation uses only one direction
}

#[allow(dead_code)]
// Complete animation mapping that preserves IDs while adding metadata
pub const ANIMATION_INFO: &[AnimationInfo] = &[
    AnimationInfo {
        id: 0,
        name: "Walk",
        source: "monster",
        max_frames: 8,
        group_index: 0,
        single_direction: false,
    },
    AnimationInfo {
        id: 1,
        name: "Attack",
        source: "m_attack",
        max_frames: 16,
        group_index: 0,
        single_direction: false,
    },
    AnimationInfo {
        id: 2,
        name: "Special_1",
        source: "m_attack",
        max_frames: 16,
        group_index: 1,
        single_direction: false,
    },
    AnimationInfo {
        id: 3,
        name: "Special_2",
        source: "m_attack",
        max_frames: 16,
        group_index: 2,
        single_direction: false,
    },
    AnimationInfo {
        id: 4,
        name: "Special_3",
        source: "m_attack",
        max_frames: 16,
        group_index: 3,
        single_direction: false,
    },
    AnimationInfo {
        id: 5,
        name: "Sleep",
        source: "monster",
        max_frames: 8,
        group_index: 1,
        single_direction: true,
    },
    AnimationInfo {
        id: 6,
        name: "Hurt",
        source: "monster",
        max_frames: 8,
        group_index: 2,
        single_direction: false,
    },
    AnimationInfo {
        id: 7,
        name: "Idle",
        source: "monster",
        max_frames: 8,
        group_index: 3,
        single_direction: false,
    },
    AnimationInfo {
        id: 8,
        name: "Swing",
        source: "m_attack",
        max_frames: 16,
        group_index: 4,
        single_direction: false,
    },
    AnimationInfo {
        id: 9,
        name: "Double",
        source: "m_attack",
        max_frames: 20,
        group_index: 5,
        single_direction: false,
    },
    AnimationInfo {
        id: 10,
        name: "Hop",
        source: "m_attack",
        max_frames: 16,
        group_index: 6,
        single_direction: false,
    },
    AnimationInfo {
        id: 11,
        name: "Charge",
        source: "monster",
        max_frames: 16,
        group_index: 4,
        single_direction: false,
    },
    AnimationInfo {
        id: 12,
        name: "Rotate",
        source: "m_attack",
        max_frames: 16,
        group_index: 7,
        single_direction: false,
    },
];

// Helper functions to access by ID or other attributes
impl AnimationInfo {
    pub fn find_by_id(id: u8) -> Option<&'static AnimationInfo> {
        ANIMATION_INFO.iter().find(|&info| info.id == id)
    }

    pub fn get_animations_for_bin(bin_name: &str) -> Vec<&'static AnimationInfo> {
        ANIMATION_INFO
            .iter()
            .filter(|&info| info.source == bin_name)
            .collect()
    }

    pub fn get_group_index_for_id(id: u8, bin_name: &str) -> Option<u8> {
        ANIMATION_INFO
            .iter()
            .find(|&info| info.id == id && info.source == bin_name)
            .map(|info| info.group_index)
    }
}

/// Animation type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationType {
    Walk = 0,
    Attack = 1,
    Special1 = 2,
    Special2 = 3,
    Special3 = 4,
    Sleep = 5,
    Hurt = 6,
    Idle = 7,
    Swing = 8,
    Double = 9,
    Hop = 10,
    Charge = 11,
    Rotate = 12,
    Unknown = 255,
}

impl From<u8> for AnimationType {
    fn from(value: u8) -> Self {
        match value {
            0 => AnimationType::Walk,
            1 => AnimationType::Attack,
            2 => AnimationType::Special1,
            3 => AnimationType::Special2,
            4 => AnimationType::Special3,
            5 => AnimationType::Sleep,
            6 => AnimationType::Hurt,
            7 => AnimationType::Idle,
            8 => AnimationType::Swing,
            9 => AnimationType::Double,
            10 => AnimationType::Hop,
            11 => AnimationType::Charge,
            12 => AnimationType::Rotate,
            _ => AnimationType::Unknown,
        }
    }
}

impl AnimationType {
    /// Get the name of this animation type
    pub fn name(&self) -> &'static str {
        match self {
            AnimationType::Walk => "Walk",
            AnimationType::Attack => "Attack",
            AnimationType::Special1 => "Special1",
            AnimationType::Special2 => "Special2",
            AnimationType::Special3 => "Special3",
            AnimationType::Sleep => "Sleep",
            AnimationType::Hurt => "Hurt",
            AnimationType::Idle => "Idle",
            AnimationType::Swing => "Swing",
            AnimationType::Double => "Double",
            AnimationType::Hop => "Hop",
            AnimationType::Charge => "Charge",
            AnimationType::Rotate => "Rotate",
            AnimationType::Unknown => "Unknown",
        }
    }
}

/// A single frame within an animation
#[derive(Debug, Clone)]
pub struct FrameData {
    /// Index in the WAN frame data
    pub frame_index: u16,
    /// Duration in game ticks
    pub duration: u8,
    /// When damage is dealt (flag 0x02)
    pub is_hit_frame: bool,
    /// When to return to idle (flag 0x01)
    pub is_return_frame: bool,
    /// For charge-up animations
    pub is_rush_frame: bool,
    /// Sprite X offset
    pub offset_x: i16,
    /// Sprite Y offset
    pub offset_y: i16,
    /// Shadow X offset
    pub shadow_offset_x: i16,
    /// Shadow Y offset
    pub shadow_offset_y: i16,
    /// Head position (x, y)
    pub head_pos: Option<(i16, i16)>,
    /// Left hand position (x, y)
    pub lhand_pos: Option<(i16, i16)>,
    /// Right hand position (x, y)
    pub rhand_pos: Option<(i16, i16)>,
    /// Center position (x, y)
    pub center_pos: Option<(i16, i16)>,
}

impl FrameData {
    /// Create a new frame data with basic details
    pub fn new(frame_index: u16, duration: u8) -> Self {
        Self {
            frame_index,
            duration,
            is_hit_frame: false,
            is_return_frame: false,
            is_rush_frame: false,
            offset_x: 0,
            offset_y: 0,
            shadow_offset_x: 0,
            shadow_offset_y: 0,
            head_pos: None,
            lhand_pos: None,
            rhand_pos: None,
            center_pos: None,
        }
    }
}

/// Animation for a specific direction (0-7)
#[derive(Debug, Clone)]
pub struct DirectionalAnim {
    /// Direction index (0-7)
    pub direction: u8,
    /// Sequence of frames
    pub frames: Vec<FrameData>,
}

impl DirectionalAnim {
    /// Create a new empty directional animation
    pub fn new(direction: u8) -> Self {
        Self {
            direction,
            frames: Vec::new(),
        }
    }

    /// Get total duration of this animation in game ticks
    pub fn total_duration(&self) -> u32 {
        self.frames.iter().map(|f| f.duration as u32).sum()
    }
}

/// Data for a specific animation type (Walk, Attack, etc.)
#[derive(Debug, Clone)]
pub struct AnimationData {
    /// Animation ID (0-12)
    pub anim_id: u8,
    /// Animation type
    pub anim_type: AnimationType,
    /// Animations for different directions
    pub directions: Vec<DirectionalAnim>,
    /// Source bin file
    pub from_bin_file: String,
}

impl AnimationData {
    /// Create a new animation data
    pub fn new(anim_id: u8, from_bin_file: &str) -> Self {
        Self {
            anim_id,
            anim_type: AnimationType::from(anim_id),
            directions: Vec::new(),
            from_bin_file: from_bin_file.to_string(),
        }
    }

    /// Add a directional animation
    pub fn add_direction(&mut self, direction: DirectionalAnim) {
        // If we already have this direction, replace it
        if let Some(pos) = self
            .directions
            .iter()
            .position(|d| d.direction == direction.direction)
        {
            self.directions[pos] = direction;
        } else {
            self.directions.push(direction);
        }
    }

    /// Get a directional animation
    pub fn get_direction(&self, direction: u8) -> Option<&DirectionalAnim> {
        self.directions.iter().find(|d| d.direction == direction)
    }
}

/// Complete animation metadata for a Pokémon
#[derive(Debug, Clone)]
pub struct PokemonAnimationMetadata {
    /// Pokémon ID in monster.md
    pub pokemon_id: usize,
    /// Sprite index
    pub sprite_index: usize,
    /// Maps anim_id -> data
    pub animations: HashMap<u8, AnimationData>,
    /// Source bin files this metadata came from
    pub source_files: Vec<String>,
}

impl PokemonAnimationMetadata {
    /// Create a new empty animation metadata
    pub fn new(pokemon_id: usize, sprite_index: usize) -> Self {
        Self {
            pokemon_id,
            sprite_index,
            animations: HashMap::new(),
            source_files: Vec::new(),
        }
    }

    /// Add an animation from a specific bin file
    pub fn add_animation_from_bin(&mut self, anim_id: u8, bin_file: &str) -> &mut AnimationData {
        // Track source file if not already added
        if !self.source_files.contains(&bin_file.to_string()) {
            self.source_files.push(bin_file.to_string());
        }

        // Create or get the animation data
        self.animations
            .entry(anim_id)
            .or_insert_with(|| AnimationData::new(anim_id, bin_file))
    }

    /// Check if an animation ID exists
    pub fn has_animation(&self, anim_id: u8) -> bool {
        self.animations.contains_key(&anim_id)
    }

    /// Get an animation by ID
    pub fn get_animation(&self, anim_id: u8) -> Option<&AnimationData> {
        self.animations.get(&anim_id)
    }

    /// Get an animation by type
    pub fn get_animation_by_type(&self, anim_type: AnimationType) -> Option<&AnimationData> {
        self.animations.get(&(anim_type as u8))
    }
}

/// Container for all Pokémon animation metadata
#[derive(Debug)]
pub struct AnimationDatabase {
    pub pokemon_animations: HashMap<usize, PokemonAnimationMetadata>,
}

impl AnimationDatabase {
    /// Create a new empty animation database
    pub fn new() -> Self {
        Self {
            pokemon_animations: HashMap::new(),
        }
    }

    /// Add a Pokémon's animation metadata
    pub fn add_pokemon(&mut self, metadata: PokemonAnimationMetadata) {
        self.pokemon_animations
            .insert(metadata.pokemon_id, metadata);
    }

    /// Get animation metadata for a Pokémon by ID
    pub fn get_pokemon(&self, pokemon_id: usize) -> Option<&PokemonAnimationMetadata> {
        self.pokemon_animations.get(&pokemon_id)
    }

    /// Get the number of Pokémon in the database
    pub fn len(&self) -> usize {
        self.pokemon_animations.len()
    }

    /// Check if the database is empty
    pub fn is_empty(&self) -> bool {
        self.pokemon_animations.is_empty()
    }
}
