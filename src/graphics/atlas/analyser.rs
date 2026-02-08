//! Frame Analysis for Atlas Generation
//!
//! Calculates optimal frame dimensions and collects frame data needed for atlas creation.
//! Uses anchor-based positioning where entity origin (feet/ground point) is tracked
//! for proper sprite alignment.

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
    /// Maximum extents from entity origin across all frames
    pub max_extent_left: i32,
    pub max_extent_right: i32,
    pub max_extent_up: i32,
    pub max_extent_down: i32,
    pub total_original_frames: usize,
}

/// Holds data extracted and calculated for a single frame during analysis.
#[derive(Debug, Clone)]
pub struct AnalysedFrame {
    pub image: RgbaImage,

    /// Entity origin X position relative to cropped content's top-left.
    /// Positive means origin is inside/right of content left edge.
    /// Negative means origin is to the left of content.
    pub entity_origin_x: i32,

    /// Entity origin Y position relative to cropped content's top-left.
    /// Positive means origin is inside/below content top edge.
    /// Negative means origin is above content.
    pub entity_origin_y: i32,

    pub source_bin: String,
    pub original_wan_frame_index: usize,

    pub original_shadow_x: i16,
    pub original_shadow_y: i16,

    pub group_idx: usize,

    pub final_placement_x: i32,
    pub final_placement_y: i32,

    /// Content bounds in the original extracted frame, before cropping
    pub content_bounds: (i32, i32, i32, i32),

    /// WAN coordinate bounds, relative to entity origin
    pub wan_bounds: (i16, i16, i16, i16),
}

/// Analyses frames from all provided WAN files for a single Pokemon
///
/// Extracts frames, calculates bounds, determines entity origin positions,
/// and maps frames back to their original animation/direction/sequence source.
pub fn analyse_frames(
    wan_files: &HashMap<String, WanFile>,
    dex_num: u16,
) -> Result<FrameAnalysis, AtlasError> {
    let mut ordered_frames = Vec::new();

    // Track maximum extents from entity origin
    let mut max_extent_left: i32 = 0;
    let mut max_extent_right: i32 = 0;
    let mut max_extent_up: i32 = 0;
    let mut max_extent_down: i32 = 0;

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
                            // HACK: Pushing null ptrs allows access to the first frame in the
                            // animation, but pushes the window one to the left meaning ignores the
                            // last walk animation frame, so we add it back in
                            let frame_index = (seq_frame.frame_index).saturating_add(1) as usize;

                            // Ignore the blank frame created due to pushing null ptrs
                            if frame_index == 0 {
                                continue;
                            }

                            if frame_index >= wan_file.frame_data.len() {
                                continue;
                            }

                            // Extract the frame image
                            let frame_image = match extract_frame(wan_file, frame_index) {
                                Ok(img) => img,
                                Err(_) => continue,
                            };

                            // Get the WAN coordinate bounds for this frame
                            let wan_bounds = get_wan_frame_bounds(wan_file, frame_index);

                            // Find content bounds in the extracted image
                            let content_bounds = find_content_bounds(&frame_image);
                            let content_width = (content_bounds.2 - content_bounds.0).max(0) as u32;
                            let content_height =
                                (content_bounds.3 - content_bounds.1).max(0) as u32;

                            // Calculate entity origin position in the extracted image
                            let origin_in_extracted_x = -(wan_bounds.0 as i32);
                            let origin_in_extracted_y = -(wan_bounds.1 as i32);

                            // After cropping, adjust for the crop offset
                            let entity_origin_x = origin_in_extracted_x - content_bounds.0;
                            let entity_origin_y = origin_in_extracted_y - content_bounds.1;

                            // Update maximum extents from entity origin
                            // extent_left = how far content extends to the LEFT of origin
                            max_extent_left = max_extent_left.max(entity_origin_x);
                            // extent_right = how far content extends to the RIGHT of origin
                            max_extent_right =
                                max_extent_right.max(content_width as i32 - entity_origin_x);
                            // extent_up = how far content extends ABOVE origin
                            max_extent_up = max_extent_up.max(entity_origin_y);
                            // extent_down = how far content extends BELOW origin
                            max_extent_down =
                                max_extent_down.max(content_height as i32 - entity_origin_y);

                            let cropped_image = if content_width > 0 && content_height > 0 {
                                image::imageops::crop_imm(
                                    &frame_image,
                                    content_bounds.0 as u32,
                                    content_bounds.1 as u32,
                                    content_width,
                                    content_height,
                                )
                                .to_image()
                            } else {
                                RgbaImage::new(1, 1)
                            };

                            ordered_frames.push((
                                anim_id,
                                dir_idx as u8,
                                seq_idx,
                                AnalysedFrame {
                                    image: cropped_image,
                                    entity_origin_x,
                                    entity_origin_y,
                                    source_bin: source_bin_name.clone(),
                                    original_wan_frame_index: frame_index,
                                    original_shadow_x: seq_frame.shadow.0,
                                    original_shadow_y: seq_frame.shadow.1,
                                    group_idx: group_id,
                                    final_placement_x: 0,
                                    final_placement_y: 0,
                                    content_bounds,
                                    wan_bounds,
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
        max_extent_left,
        max_extent_right,
        max_extent_up,
        max_extent_down,
    })
}

/// Gets the WAN coordinate bounds for a frame, bounds relative to entity origin
fn get_wan_frame_bounds(wan: &WanFile, frame_idx: usize) -> (i16, i16, i16, i16) {
    const TEX_SIZE: usize = 8;

    if frame_idx >= wan.frame_data.len() {
        return (0, 0, 0, 0);
    }

    let frame = &wan.frame_data[frame_idx];
    if frame.pieces.is_empty() {
        return (0, 0, 0, 0);
    }

    let mut bounds = (i16::MAX, i16::MAX, i16::MIN, i16::MIN);

    for piece in &frame.pieces {
        let (width_blocks, height_blocks) = piece.get_dimensions();
        let width_px = (width_blocks * TEX_SIZE) as i16;
        let height_px = (height_blocks * TEX_SIZE) as i16;

        bounds.0 = bounds.0.min(piece.x_offset);
        bounds.1 = bounds.1.min(piece.y_offset);
        bounds.2 = bounds.2.max(piece.x_offset + width_px);
        bounds.3 = bounds.3.max(piece.y_offset + height_px);
    }

    if bounds.0 == i16::MAX {
        return (0, 0, 0, 0);
    }

    bounds
}

/// Calculates the optimal frame size for the atlas based on FrameAnalysis
/// Uses dynamic sizing based on maximum extents from entity origin
pub fn calculate_optimal_size(analysis: &FrameAnalysis, config: &AtlasConfig) -> (u32, u32) {
    // Calculate required size based on maximum extents from entity origin
    let width_needed = (analysis.max_extent_left + analysis.max_extent_right) as u32;
    let height_needed = (analysis.max_extent_up + analysis.max_extent_down) as u32;

    // Add padding
    let width_with_padding = width_needed + config.offset_padding as u32 * 2;
    let height_with_padding = height_needed + config.offset_padding as u32 * 2;

    // Ensure minimum dimensions
    let final_width = width_with_padding.max(config.min_frame_width);
    let final_height = height_with_padding.max(config.min_frame_height);

    (
        round_up_to_multiple_of_8(final_width),
        round_up_to_multiple_of_8(final_height),
    )
}

/// Calculates the anchor point position within the frame cell
/// The anchor is where the entity origin (feet/ground point) will be placed
pub fn calculate_anchor_point(
    analysis: &FrameAnalysis,
    frame_width: u32,
    frame_height: u32,
) -> (i32, i32) {
    let anchor_x = analysis.max_extent_left
        + (frame_width as i32 - analysis.max_extent_left - analysis.max_extent_right) / 2;

    let anchor_y = analysis.max_extent_up
        + (frame_height as i32 - analysis.max_extent_up - analysis.max_extent_down) / 2;

    (anchor_x, anchor_y)
}

/// Finds the bounding box of non-transparent pixels in an image
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
