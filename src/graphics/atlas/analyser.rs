//! Frame Analysis for Atlas Generation
//!
//! Calculates optimal frame dimensions and collects frame data needed for atlas creation.

use crate::{
    data::animation_metadata as AmData,
    graphics::{
        atlas::{AtlasConfig, AtlasError},
        wan::{renderer::extract_frame, WanFile},
    },
};

use image::RgbaImage;
use std::collections::HashMap;

/// Holds the results of analysing all frames for a single Pokémon.
#[derive(Debug)]
pub struct FrameAnalysis {
    pub dex_num: u16,
    pub ordered_frames: Vec<(u8, u8, usize, AnalysedFrame)>,
    pub max_content_size: (u32, u32),
    pub total_original_frames: usize,
}

/// Holds data extracted and calculated for a single frame during analysis.
#[derive(Debug, Clone)]
pub struct AnalysedFrame {
    pub image: RgbaImage,
    pub ref_offset_x: i32,
    pub ref_offset_y: i32,

    pub source_bin: String,
    pub original_wan_frame_index: usize,

    pub original_shadow_x: i16,
    pub original_shadow_y: i16,

    pub group_idx: usize,

    pub final_placement_x: i32,
    pub final_placement_y: i32,
}

/// Analyses frames from all provided WAN files for a single Pokémon.
///
/// Extracts frames, calculates bounds, determines max offsets, and maps
/// frames back to their original animation/direction/sequence source.
pub fn analyse_frames(
    wan_files: &HashMap<String, WanFile>,
    dex_num: u16,
) -> Result<FrameAnalysis, AtlasError> {
    let mut ordered_frames = Vec::new();
    let mut max_content_width: u32 = 0;
    let mut max_content_height: u32 = 0;

    // Iterate through monster, m_attack binary files
    let mut sorted_wan_keys: Vec<&String> = wan_files.keys().collect();
    sorted_wan_keys.sort();

    for source_bin_name in sorted_wan_keys {
        let wan_file = &wan_files[source_bin_name];
        let source_bin_simple = if source_bin_name.contains("monster") {
            "monster"
        } else {
            "m_attack"
        };

        // Iterate through the *indices* of the animation_groups vector (0 up to len-1)
        for potential_anim_id_usize in 0..wan_file.animation_groups.len() {
            let potential_anim_id = potential_anim_id_usize as u8;

            // Check if this anim_id belongs to the source file
            if AmData::AnimationInfo::find_by_id_and_source(potential_anim_id, source_bin_simple)
                .is_none()
            {
                // This anim_id not in ANIMATION_INFO
                if !wan_file.animation_groups[potential_anim_id_usize].is_empty()
                    && (source_bin_name != "m_attack.bin" && potential_anim_id_usize != 11)
                {
                    println!(
                        "Warning: No semantic mapping found for group {} in {}. Skipping.",
                        potential_anim_id_usize, source_bin_name
                    );
                }
                continue;
            }

            // Now get the actual animation group using the index (which is the semantic ID)
            let anim_group = &wan_file.animation_groups[potential_anim_id_usize];

            if anim_group.is_empty() {
                continue;
            }

            let semantic_anim_id = potential_anim_id;

            // Iterate through directions
            for (dir_idx_u8, direction_anim) in anim_group.iter().enumerate() {
                let dir_idx = dir_idx_u8 as u8;

                // Iterate through frames in the sequence
                for (sequence_idx, seq_frame) in direction_anim.frames.iter().enumerate() {
                    let original_wan_frame_index = seq_frame.frame_index as usize;

                    if original_wan_frame_index >= wan_file.frame_data.len() {
                        eprintln!(
                            "Error: Invalid frame index {} referenced by anim {}, dir {}, seq {} (max: {}). Skipping frame.",
                            original_wan_frame_index, semantic_anim_id, dir_idx, sequence_idx, wan_file.frame_data.len() - 1
                        );
                        continue;
                    }

                    // Extract the raw frame image using the renderer
                    let frame_image = match extract_frame(wan_file, original_wan_frame_index) {
                        Ok(img) => img,
                        Err(e) => {
                            eprintln!(
                                "Error extracting frame {} (anim {}, dir {}, seq {}): {}",
                                original_wan_frame_index,
                                semantic_anim_id,
                                dir_idx,
                                sequence_idx,
                                e
                            );
                            continue;
                        }
                    };

                    // Find content bounds for non-transparent pixels
                    let bounds = find_content_bounds(&frame_image);
                    let content_width = if bounds.2 > bounds.0 {
                        (bounds.2 - bounds.0) as u32
                    } else {
                        0
                    };
                    let content_height = if bounds.3 > bounds.1 {
                        (bounds.3 - bounds.1) as u32
                    } else {
                        0
                    };

                    let cropped_image = if content_width > 0 && content_height > 0 {
                        image::imageops::crop_imm(
                            &frame_image,
                            bounds.0 as u32,
                            bounds.1 as u32,
                            content_width,
                            content_height,
                        )
                        .to_image()
                    } else {
                        RgbaImage::new(1, 1)
                    };

                    max_content_width = max_content_width.max(content_width);
                    max_content_height = max_content_height.max(content_height);

                    let content_ref_x = content_width as i32 / 2;
                    let ref_offset_x = bounds.0 + content_ref_x;
                    let ref_offset_y = bounds.1 + (content_height as f32 * 0.75) as i32;

                    ordered_frames.push((
                        semantic_anim_id,
                        dir_idx,
                        sequence_idx,
                        AnalysedFrame {
                            image: cropped_image,
                            ref_offset_x,
                            ref_offset_y,
                            source_bin: source_bin_name.clone(),
                            original_wan_frame_index,
                            original_shadow_x: seq_frame.shadow.0,
                            original_shadow_y: seq_frame.shadow.1,
                            group_idx: potential_anim_id_usize,
                            final_placement_x: 0,
                            final_placement_y: 0,
                        },
                    ));
                }
            }
        }
    }

    Ok(FrameAnalysis {
        dex_num,
        total_original_frames: ordered_frames.len(),
        ordered_frames,
        max_content_size: (max_content_width, max_content_height),
    })
}

/// Calculates the optimal frame size for the atlas based on analysis results.
pub fn calculate_optimal_size(analysis: &FrameAnalysis, config: &AtlasConfig) -> (u32, u32) {
    let (max_content_width, max_content_height) = analysis.max_content_size;

    // Calculate size needed based on content and basic padding
    let width_needed = max_content_width + config.offset_padding as u32 * 2;
    let height_needed = max_content_height + config.offset_padding as u32 * 2;

    // Apply minimum dimensions
    let final_width = width_needed.max(config.min_frame_width);
    let final_height = height_needed.max(config.min_frame_height);

    (
        round_up_to_multiple_of_8(final_width),
        round_up_to_multiple_of_8(final_height),
    )
}

/// Finds the bounding box of non-transparent pixels in an image.
fn find_content_bounds(image: &RgbaImage) -> (i32, i32, i32, i32) {
    let (width, height) = image.dimensions();
    let mut min_x = width as i32;
    let mut min_y = height as i32;
    let mut max_x = -1;
    let mut max_y = -1;

    for y in 0..height {
        for x in 0..width {
            if image.get_pixel(x, y)[3] > 0 {
                // Non-transparent pixel found
                min_x = min_x.min(x as i32);
                min_y = min_y.min(y as i32);
                max_x = max_x.max(x as i32);
                max_y = max_y.max(y as i32);
            }
        }
    }

    // Image was entirely transparent
    if max_x < min_x || max_y < min_y {
        (0, 0, 0, 0)
    } else {
        (min_x, min_y, max_x + 1, max_y + 1) // max is exclusive
    }
}

/// Rounds an integer up to the nearest multiple of 8.
pub fn round_up_to_multiple_of_8(n: u32) -> u32 {
    if n == 0 {
        8
    } else {
        ((n + 7) / 8) * 8
    }
}
