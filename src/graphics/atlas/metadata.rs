//! Metadata Generation for Sprite Atlases
//!
//! Creates a JSON file describing the atlas layout, animations,
//! directions, and frame properties.

use std::{collections::HashMap, fs::File, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    data::animation_metadata as AmData,
    graphics::{
        atlas::{analyser::FrameAnalysis, generator::AtlasLayout},
        wan::{AnimationStructure, WanFile},
    },
};

const SINGLE_DIRECTION_ANIMATIONS: &[u8] = &[5];

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AtlasMetadata {
    /// Filename of the atlas PNG image this metadata corresponds to
    pub atlas_image: String,
    pub frame_width: u32,
    pub frame_height: u32,
    pub total_frames_in_atlas: u32,
    pub shadow_size: u8,
    pub animations: HashMap<String, AtlasAnimationInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AtlasAnimationInfo {
    pub anim_id: u8,
    pub name: String,
    pub source_bin: String,
    pub directions: Vec<DirectionInfo>,
    /// Only used for Sleep animation group
    pub single_direction: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DirectionInfo {
    pub direction: u8,
    pub frames: Vec<FrameInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FrameInfo {
    /// Index of this frame within the unique frames of the atlas sheet.
    pub idx: u32,
    /// Top left X coordinate of this frame's cell in the atlas sheet (in pixels).
    pub sheet_x: u32,
    /// Top left Y coordinate of this frame's cell in the atlas sheet (in pixels).
    pub sheet_y: u32,
    /// Duration this frame is displayed (in game ticks, typically 1/60th sec).
    pub duration: u8,
    /// X offset to apply when drawing, relative to the standard reference point (feet).
    pub offset_x: i32,
    /// Y offset to apply when drawing, relative to the standard reference point (feet).
    pub offset_y: i32,
    /// X offset for placing the shadow sprite, relative to the standard reference point.
    pub shadow_offset_x: i32,
    /// Y offset for placing the shadow sprite, relative to the standard reference point.
    pub shadow_offset_y: i32,
    /// Head position relative to the frame cell's top-left (0,0).
    pub head_pos: Option<[i32; 2]>,
    /// Left hand position relative to the frame cell's top-left (0,0).
    pub lhand_pos: Option<[i32; 2]>,
    /// Right hand position relative to the frame cell's top-left (0,0).
    pub rhand_pos: Option<[i32; 2]>,
    /// Centre position relative to the frame cell's top-left (0,0).
    pub centre_pos: Option<[i32; 2]>,
    pub is_hit_frame: bool,
    /// True if the animation should return to idle after this frame.
    pub is_return_frame: bool,
    /// True if this is a key frame in a charge-up/multi-hit sequence.
    pub is_rush_frame: bool,
}

/// Generates the complete AtlasMetadata structure
pub fn generate_metadata(
    wan_files: &HashMap<String, WanFile>,
    analysis: &FrameAnalysis,
    frame_width: u32,
    frame_height: u32,
    layout: &AtlasLayout,
    frame_mapping: &[usize],
    shadow_size: u8,
) -> Result<AtlasMetadata, super::AtlasError> {
    let mut output_animations: HashMap<String, AtlasAnimationInfo> = HashMap::new();
    let total_unique_frames = frame_mapping.iter().max().map_or(0, |&max_idx| max_idx + 1);

    for (original_global_index, (anim_id, dir_idx, sequence_idx, analysed_frame)) in
        analysis.ordered_frames.iter().enumerate()
    {
        let unique_atlas_index = frame_mapping[original_global_index];
        let unique_atlas_index_u32 = unique_atlas_index as u32;

        let atlas_col = (unique_atlas_index % layout.frames_per_row as usize) as u32;
        let atlas_row = (unique_atlas_index / layout.frames_per_row as usize) as u32;
        let sheet_x = atlas_col * frame_width;
        let sheet_y = atlas_row * frame_height;

        let animation_info = match AmData::AnimationInfo::find_by_id(*anim_id) {
            Some(info) => info,
            None => {
                if *anim_id > 12 {
                    continue;
                }
                return Err(super::AtlasError::MetadataError(format!(
                    "Unknown anim_id {} in standard range",
                    *anim_id
                )));
            }
        };

        let wan_file = wan_files.get(&analysed_frame.source_bin).ok_or_else(|| {
            super::AtlasError::MetadataError(format!(
                "Could not find WAN file for source '{}'",
                analysed_frame.source_bin
            ))
        })?;

        // Access the Character animation structure
        let original_seq_frame = match &wan_file.animations {
            AnimationStructure::Character(groups) => {
                groups
                    .get(analysed_frame.group_idx) // Get the animation group
                    .and_then(|group| group.get(*dir_idx as usize)) // Get the direction
                    .and_then(|anim| anim.frames.get(*sequence_idx)) // Get the frame
            }
            AnimationStructure::Effect(_) => None,
        }
        .ok_or_else(|| {
            super::AtlasError::MetadataError(format!(
                "Could not find original WAN sequence frame for {} anim {}, dir {}, seq {}",
                analysed_frame.source_bin, anim_id, dir_idx, sequence_idx
            ))
        })?;

        let frame_offset_data = wan_file
            .body_part_offset_data
            .get(analysed_frame.original_wan_frame_index);

        let original_ref_x = analysed_frame.ref_offset_x;
        let original_ref_y = analysed_frame.ref_offset_y;

        let adjust_offset_relative = |orig_offset: Option<(i16, i16)>| -> Option<[i32; 2]> {
            orig_offset.map(|(ox, oy)| [ox as i32 - original_ref_x, oy as i32 - original_ref_y])
        };

        let head_pos_rel = adjust_offset_relative(frame_offset_data.map(|fod| fod.head));
        let lhand_pos_rel = adjust_offset_relative(frame_offset_data.map(|fod| fod.lhand));
        let rhand_pos_rel = adjust_offset_relative(frame_offset_data.map(|fod| fod.rhand));
        let centre_pos_rel = adjust_offset_relative(frame_offset_data.map(|fod| fod.centre));

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
            centre_pos: centre_pos_rel,
        };

        let anim_output_info = output_animations
            .entry(animation_info.name.to_string())
            .or_insert_with(|| AtlasAnimationInfo {
                anim_id: *anim_id,
                name: animation_info.name.to_string(),
                source_bin: analysed_frame.source_bin.clone(),
                directions: Vec::new(),
                single_direction: SINGLE_DIRECTION_ANIMATIONS.contains(anim_id),
            });

        let dir_output_info = match anim_output_info
            .directions
            .iter_mut()
            .find(|d| d.direction == *dir_idx)
        {
            Some(d) => d,
            None => {
                anim_output_info.directions.push(DirectionInfo {
                    direction: *dir_idx,
                    frames: Vec::new(),
                });
                anim_output_info.directions.sort_by_key(|d| d.direction);
                anim_output_info
                    .directions
                    .iter_mut()
                    .find(|d| d.direction == *dir_idx)
                    .unwrap()
            }
        };

        dir_output_info.frames.push(frame_info);
    }

    Ok(AtlasMetadata {
        atlas_image: format!("{:03}_atlas.png", analysis.dex_num),
        frame_width,
        frame_height,
        total_frames_in_atlas: total_unique_frames as u32,
        shadow_size,
        animations: output_animations,
    })
}

/// Saves the generated AtlasMetadata to a JSON file
pub fn save_metadata(metadata: &AtlasMetadata, path: &Path) -> Result<(), super::AtlasError> {
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, metadata)?;
    Ok(())
}
