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
    Reuse(ReuseEffect),
    Screen(ScreenEffect),
}

/// Defines a visual effect that is rendered from a sprite sheet
#[derive(Serialize, Debug)]
pub struct SpriteEffect {
    #[serde(rename = "sprite_sheet")]
    pub sprite_sheet: String,
    #[serde(rename = "frame_width")]
    pub frame_width: u32,
    #[serde(rename = "frame_height")]
    pub frame_height: u32,
    pub animations: HashMap<String, AnimationSequence>,
    pub is_directional: bool,
    pub direction_count: u8,
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

/// Defines an effect that reuses an existing Pokemon's animation
#[derive(Serialize, Debug)]
pub struct ReuseEffect {
    pub target: String,
    #[serde(rename = "animation_index")]
    pub animation_index: u32,
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

/// Describes an effect that is triggered by a move
#[derive(Serialize, Debug)]
pub struct MoveEffectTrigger {
    pub id: String,      // The effect_id to play
    pub trigger: String, // For now, this is always "OnExecute"
}
