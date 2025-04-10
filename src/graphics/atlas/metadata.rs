//! Metadata Generation for Sprite Atlases
//!
//! Creates a JSON file describing the atlas layout, animations,
//! directions, and frame properties.

use super::analyser::{AnalysedFrame, FrameAnalysis};
use super::generator::AtlasLayout;
use crate::data::animation_metadata as AmData; // Alias
use crate::graphics::wan::WanFile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

// --- Metadata Struct Definitions ---

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AtlasMetadata {
    /// Filename of the atlas PNG image this metadata corresponds to.
    pub atlas_image: String,
    /// Width of each individual frame cell in the atlas.
    pub frame_width: u32,
    /// Height of each individual frame cell in the atlas.
    pub frame_height: u32,
    /// Total number of unique frames present in the atlas image.
    pub total_frames_in_atlas: u32, // Renamed for clarity
    /// Shadow size category (0=small, 1=medium, 2=large).
    pub shadow_size: u8,
    /// Animation data, keyed by semantic animation name (e.g., "Walk", "Attack").
    pub animations: HashMap<String, AnimationInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnimationInfo {
    /// The semantic Animation ID (e.g., 0 for Walk, 1 for Attack).
    pub anim_id: u8,
    /// The semantic name of the animation (e.g., "Walk").
    pub name: String,
    /// The original source bin file ("monster", "m_attack").
    pub source_bin: String,
    /// List of directions available for this animation.
    pub directions: Vec<DirectionInfo>,
    /// True if this animation only uses the 'down' direction (e.g., Sleep).
    pub single_direction: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DirectionInfo {
    /// Direction index (0=down, 1=down-right, ..., 7=down-left).
    pub direction: u8,
    /// Sequence of frames for this direction.
    pub frames: Vec<FrameInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FrameInfo {
    /// Index of this frame within the **unique frames** of the atlas sheet.
    pub idx: u32,
    /// Top-left X coordinate of this frame's cell in the atlas sheet (in pixels).
    pub sheet_x: u32,
    /// Top-left Y coordinate of this frame's cell in the atlas sheet (in pixels).
    pub sheet_y: u32,
    /// Duration this frame is displayed (in game ticks, typically 1/60th sec).
    pub duration: u8,
    /// X offset to apply when drawing, relative to the standard reference point (feet).
    pub offset_x: i32,
    /// Y offset to apply when drawing, relative to the standard reference point (feet).
    /// Positive values typically mean move UP in engine coordinate systems.
    pub offset_y: i32,
    /// X offset for placing the shadow sprite, relative to the standard reference point.
    pub shadow_offset_x: i32,
    /// Y offset for placing the shadow sprite, relative to the standard reference point.
    pub shadow_offset_y: i32,
    /// Optional head position relative to the frame cell's top-left (0,0).
    pub head_pos: Option<[i32; 2]>,
    /// Optional left hand position relative to the frame cell's top-left (0,0).
    pub lhand_pos: Option<[i32; 2]>,
    /// Optional right hand position relative to the frame cell's top-left (0,0).
    pub rhand_pos: Option<[i32; 2]>,
    /// Optional center position relative to the frame cell's top-left (0,0).
    pub center_pos: Option<[i32; 2]>,
    /// True if damage calculation should occur on this frame.
    pub is_hit_frame: bool,
    /// True if the animation should return to idle after this frame.
    pub is_return_frame: bool,
    /// True if this is a key frame in a charge-up/multi-hit sequence.
    pub is_rush_frame: bool,
}

// --- Metadata Generation Function ---

/// Generates the complete AtlasMetadata structure.
pub fn generate_metadata(
    wan_files: &HashMap<String, WanFile>, // Needed to get original SequenceFrame/OffsetData
    analysis: &FrameAnalysis,             // Contains AnalysedFrame data in order
    frame_width: u32,
    frame_height: u32,
    layout: &AtlasLayout,
    frame_mapping: &[usize], // Maps original_index -> unique_atlas_index
    shadow_size: u8,
) -> Result<AtlasMetadata, super::AtlasError> {
    let mut output_animations: HashMap<String, AnimationInfo> = HashMap::new();
    let total_unique_frames = frame_mapping.iter().max().map_or(0, |&max_idx| max_idx + 1);

    // Iterate through the original frame sequence preserved in analysis.ordered_frames
    for (original_global_index, (anim_id, dir_idx, sequence_idx, analysed_frame)) in
        analysis.ordered_frames.iter().enumerate()
    {
        // --- 1. Get Info about the Unique Frame in the Atlas ---
        let unique_atlas_index = frame_mapping[original_global_index];
        let unique_atlas_index_u32 = unique_atlas_index as u32;

        // Calculate atlas sheet position for this unique frame
        let atlas_col = (unique_atlas_index % layout.frames_per_row as usize) as u32;
        let atlas_row = (unique_atlas_index / layout.frames_per_row as usize) as u32;
        let sheet_x = atlas_col * frame_width;
        let sheet_y = atlas_row * frame_height;

        // --- 2. Get Original Data for this specific frame in sequence ---
        let am_info = AmData::AnimationInfo::find_by_id(*anim_id).ok_or_else(|| {
            super::AtlasError::MetadataError(format!("Unknown anim_id {}", *anim_id))
        })?;

        let wan_file = wan_files.get(&analysed_frame.source_bin).ok_or_else(|| {
            super::AtlasError::MetadataError(format!(
                "Could not find WAN file for source '{}'",
                analysed_frame.source_bin
            ))
        })?;

        // Retrieve the original SequenceFrame
        let original_seq_frame = wan_file
            .animation_groups
            .get(analysed_frame.group_idx) // Use group_idx directly
            .and_then(|group| group.get(*dir_idx as usize))
            .and_then(|dir_anim| dir_anim.frames.get(*sequence_idx))
            .ok_or_else(|| {
                super::AtlasError::MetadataError(format!(
                    "Could not find original WAN sequence frame for {} anim {}, dir {}, seq {}",
                    analysed_frame.source_bin, anim_id, dir_idx, sequence_idx
                ))
            })?;

        // Retrieve original body part offsets (optional)
        let frame_offset_data = wan_file
            .offset_data
            .get(analysed_frame.original_wan_frame_index);

        // --- 3. UPDATED: Calculate body part positions RELATIVE TO REFERENCE POINT ---
        // For tight frames, we calculate relative to the original reference point
        // instead of adjusting to frame top-left position

        // Ref point calculated during analysis relative to original uncropped frame
        let original_ref_x = analysed_frame.ref_offset_x;
        let original_ref_y = analysed_frame.ref_offset_y;

        // Function to calculate body part position relative to reference point
        let adjust_offset_relative = |orig_offset: Option<(i16, i16)>| -> Option<[i32; 2]> {
            orig_offset.map(|(ox, oy)| {
                [
                    ox as i32 - original_ref_x, // X distance from ref point
                    oy as i32 - original_ref_y, // Y distance from ref point (in Y-Down convention)
                ]
            })
        };

        let head_pos_rel = adjust_offset_relative(frame_offset_data.map(|fod| fod.head));
        let lhand_pos_rel = adjust_offset_relative(frame_offset_data.map(|fod| fod.lhand));
        let rhand_pos_rel = adjust_offset_relative(frame_offset_data.map(|fod| fod.rhand));
        let center_pos_rel = adjust_offset_relative(frame_offset_data.map(|fod| fod.center));

        // --- 4. Create FrameInfo (keeping the same field names) ---
        let frame_info = FrameInfo {
            idx: unique_atlas_index_u32,
            sheet_x,
            sheet_y,
            duration: original_seq_frame.duration,
            offset_x: original_seq_frame.offset.0 as i32,
            offset_y: original_seq_frame.offset.1 as i32,
            shadow_offset_x: analysed_frame.original_shadow_x as i32,
            shadow_offset_y: analysed_frame.original_shadow_y as i32,
            is_hit_frame: original_seq_frame.is_hit_point(),
            is_return_frame: original_seq_frame.is_return_point(),
            is_rush_frame: original_seq_frame.is_rush_point(),
            head_pos: head_pos_rel,
            lhand_pos: lhand_pos_rel,
            rhand_pos: rhand_pos_rel,
            center_pos: center_pos_rel,
        };

        // --- 5. Add FrameInfo to the Correct Animation/Direction ---
        let anim_output_info = output_animations
            .entry(am_info.name.to_string())
            .or_insert_with(|| AnimationInfo {
                anim_id: *anim_id,
                name: am_info.name.to_string(),
                source_bin: analysed_frame.source_bin.clone(),
                directions: Vec::new(),
                single_direction: am_info.single_direction,
            });

        println!("META: Processing OrigIdx={}, UniqueIdx={}, Anim={}, Dir={}, Seq={}, Duration={}",
            original_global_index, unique_atlas_index_u32, *anim_id, *dir_idx, *sequence_idx, frame_info.duration);

        // Find or create the DirectionInfo
        let dir_output_info = match anim_output_info
            .directions
            .iter_mut()
            .find(|d| d.direction == *dir_idx)
        {
            Some(d) => d,
            None => {
                anim_output_info.directions.push(DirectionInfo {
                    direction: *dir_idx,
                    frames: Vec::with_capacity(original_seq_frame.duration as usize), // Pre-allocate roughly
                });
                // Sort directions after adding a new one for consistent output order
                anim_output_info.directions.sort_by_key(|d| d.direction);
                // Find the newly added direction again after sorting
                anim_output_info
                    .directions
                    .iter_mut()
                    .find(|d| d.direction == *dir_idx)
                    .unwrap()
            }
        };

        // Log BEFORE the check/push
        println!("  DirInfo frames current len: {}, Expected SeqIdx: {}", dir_output_info.frames.len(), *sequence_idx);

        // Ensure frames are added in the correct sequence order
        if dir_output_info.frames.len() == *sequence_idx {
            dir_output_info.frames.push(frame_info);
        } else {
            // This case indicates a potential issue in the ordered_frames generation or iteration
            eprintln!(
                "Metadata Error: Frame sequence mismatch for anim {}, dir {}. Expected index {}, found {}. Appending anyway.",
                *anim_id, *dir_idx, dir_output_info.frames.len(), *sequence_idx
            );
            // As a fallback, just push. This might mess up animation order if the warning appears.
            dir_output_info.frames.push(frame_info);
        }

        // Log AFTER push
        println!("  DirInfo frames NEW len: {}", dir_output_info.frames.len());
    }

    Ok(AtlasMetadata {
        atlas_image: format!("{:03}_atlas.png", analysis.dex_num), // Use Dex number
        frame_width,
        frame_height,
        total_frames_in_atlas: total_unique_frames as u32,
        shadow_size,
        animations: output_animations,
    })
}

/// Saves the generated AtlasMetadata to a JSON file.
pub fn save_metadata(metadata: &AtlasMetadata, path: &Path) -> Result<(), super::AtlasError> {
    let file = File::create(path)?;
    // Use serde_json::to_writer_pretty for readable output
    serde_json::to_writer_pretty(file, metadata)?;
    Ok(())
}
