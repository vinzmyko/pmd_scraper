use serde::Serialize;
use std::collections::HashMap;

/// Contains all effect definitions and move-to-effect mappings
#[derive(Serialize, Debug)]
pub struct MoveEffectsIndex {
    pub effects: HashMap<String, EffectDefinition>,
    pub moves: HashMap<String, MoveData>,
}

impl MoveEffectsIndex {
    pub fn new() -> Self {
        MoveEffectsIndex {
            effects: HashMap::new(),
            moves: HashMap::new(),
        }
    }
}

/// An enum representing the different types of effect definitions
#[derive(Serialize, Debug)]
#[serde(tag = "type")]
pub enum EffectDefinition {
    Sprite(SpriteEffect),
    Screen(ScreenEffect),
}

/// Defines a visual effect that is rendered from a sprite sheet
#[derive(Serialize, Debug)]
pub struct SpriteEffect {
    /// Path to sprite sheet. For directional effects, this is the base path
    /// and actual sheets are at `{base}_dir{0-7}.png`
    #[serde(rename = "sprite_sheet")]
    pub sprite_sheet: String,
    #[serde(rename = "frame_width")]
    pub frame_width: u32,
    #[serde(rename = "frame_height")]
    pub frame_height: u32,
    pub animations: HashMap<String, AnimationSequence>,
    /// True if effect has 8 pre-rotated directional variants (sequence_count % 8 == 0)
    /// Determined by: wan_file.max_sequences_per_group % 8 == 0
    pub is_directional: bool,
    /// Number of direction variants (8 for directional, 1 for non-directional)
    pub direction_count: u8,
    /// Base animation index from effect_animation_info. For directional effects,
    /// the ROM adds direction (0-7) to this value to select the correct sequence.
    #[serde(rename = "base_animation_index")]
    pub base_animation_index: u32,
    /// If true, game continues without waiting for animation to complete
    pub is_non_blocking: bool,
}

/// Defines a sequence of animation frames
#[derive(Serialize, Debug)]
pub struct AnimationSequence {
    #[serde(rename = "loop")]
    pub looping: bool,
    #[serde(flatten)]
    pub details: AnimationDetails,
}

/// Contains the frame-by-frame timing and offset data for an animation
#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum AnimationDetails {
    Simple {
        #[serde(rename = "frame_count")]
        frame_count: usize,
        duration: f32,
    },
    Complex {
        // Vec of [duration_seconds, offsetX, offsetY]
        frames: Vec<[f32; 3]>,
    },
}

/// Defines a screen-wide visual effect
#[derive(Serialize, Debug)]
pub struct ScreenEffect {
    #[serde(rename = "effect_name")]
    pub effect_name: String,
}

/// Defines the effects associated with a particular move
#[derive(Serialize, Debug)]
pub struct MoveData {
    pub effects: Vec<MoveEffectTrigger>,
}

/// Layer purpose based on ROM reverse engineering findings
#[derive(Serialize, Debug, Clone, Copy)]
pub enum EffectLayer {
    /// Layer 0 (offset 0x00): Charge-up, preparation effects
    Charge = 0,
    /// Layer 1 (offset 0x02): Secondary impacts, multi-hit effects
    Secondary = 1,
    /// Layer 2 (offset 0x04): Primary visual effect
    Primary = 2,
    /// Layer 3 (offset 0x06): Projectile, additional effects
    Projectile = 3,
}

/// Describes an effect that is triggered by a move
#[derive(Serialize, Debug)]
pub struct MoveEffectTrigger {
    pub id: String,
    pub layer: EffectLayer,
    pub trigger: String,
}
