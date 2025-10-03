//! Frame Analysis for Atlas Generation
//!
//! Calculates optimal frame dimensions and collects frame data needed for atlas creation.

use image::RgbaImage;
use std::collections::HashMap;

use crate::graphics::{
    atlas::{AtlasConfig, AtlasError},
    wan::{renderer::extract_frame, AnimationStructure, WanFile},
};

/// Holds the results of analysing all frames for a single Pokemon.
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

/// Analyses frames from all provided WAN files for a single Pokemon
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

    for (source_bin_name, wan_file) in wan_files {
        match &wan_file.animations {
            AnimationStructure::Character(animation_groups) => {
                const MAX_STANDARD_ANIMATIONS: usize = 13;

                for (group_id, group) in animation_groups.iter().enumerate() {
                    if group_id >= MAX_STANDARD_ANIMATIONS {
                        continue;
                    }
                    let anim_id = group_id as u8;
                    if group.is_empty() {
                        continue;
                    }
                    for (dir_idx, direction_anim) in group.iter().enumerate() {
                        for (seq_idx, seq_frame) in direction_anim.frames.iter().enumerate() {
                            let frame_index = seq_frame.frame_index as usize;

                            if frame_index >= wan_file.frame_data.len() {
                                continue;
                            }

                            let frame_image = match extract_frame(wan_file, frame_index) {
                                Ok(img) => img,
                                Err(_) => continue,
                            };

                            let bounds = find_content_bounds(&frame_image);
                            let content_width = (bounds.2 - bounds.0).max(0) as u32;
                            let content_height = (bounds.3 - bounds.1).max(0) as u32;

                            max_content_width = max_content_width.max(content_width);
                            max_content_height = max_content_height.max(content_height);

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

                            let ref_offset_x = bounds.0 + (content_width as i32 / 2);
                            let ref_offset_y = bounds.1 + (content_height as f32 * 0.75) as i32;

                            ordered_frames.push((
                                anim_id,
                                dir_idx as u8,
                                seq_idx,
                                AnalysedFrame {
                                    image: cropped_image,
                                    ref_offset_x,
                                    ref_offset_y,
                                    source_bin: source_bin_name.clone(),
                                    original_wan_frame_index: frame_index,
                                    original_shadow_x: seq_frame.shadow.0,
                                    original_shadow_y: seq_frame.shadow.1,
                                    group_idx: group_id,
                                    final_placement_x: 0,
                                    final_placement_y: 0,
                                },
                            ));
                        }
                    }
                }
            }
            AnimationStructure::Effect(_) => {
                eprintln!(
                    "Warning: Effect animation structure found in character sprite for {}",
                    source_bin_name
                );
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

/// Calculates the optimal frame size for the atlas based on FrameAnalysis
pub fn calculate_optimal_size(analysis: &FrameAnalysis, config: &AtlasConfig) -> (u32, u32) {
    let (max_content_width, max_content_height) = analysis.max_content_size;

    let width_needed = max_content_width + config.offset_padding as u32 * 2;
    let height_needed = max_content_height + config.offset_padding as u32 * 2;

    let final_width = width_needed.max(config.min_frame_width);
    let final_height = height_needed.max(config.min_frame_height);

    (
        round_up_to_multiple_of_8(final_width),
        round_up_to_multiple_of_8(final_height),
    )
}

/// Finds the bounding box of non transparent pixels in an image
fn find_content_bounds(image: &RgbaImage) -> (i32, i32, i32, i32) {
    let (width, height) = image.dimensions();
    let mut min_x = width as i32;
    let mut min_y = height as i32;
    let mut max_x = -1;
    let mut max_y = -1;

    for y in 0..height {
        for x in 0..width {
            if image.get_pixel(x, y)[3] > 0 {
                min_x = min_x.min(x as i32);
                min_y = min_y.min(y as i32);
                max_x = max_x.max(x as i32);
                max_y = max_y.max(y as i32);
            }
        }
    }

    if max_x < min_x || max_y < min_y {
        (0, 0, 0, 0)
    } else {
        (min_x, min_y, max_x + 1, max_y + 1)
    }
}

/// Rounds an integer up to the nearest multiple of 8 for optimisation
pub fn round_up_to_multiple_of_8(n: u32) -> u32 {
    if n == 0 {
        8
    } else {
        n.div_ceil(8) * 8
    }
}
