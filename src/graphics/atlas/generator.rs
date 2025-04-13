//! Atlas image generation and frame preparation.
//!
//! Handles layout calculation, frame positioning, deduplication,
//! and final atlas image creation.

use crate::graphics::atlas::analyser::FrameAnalysis;

use std::{
    collections::{HashMap, hash_map::Entry},
    hash::{Hash, Hasher},
};

use image::{imageops, RgbaImage};
use twox_hash::XxHash64;

#[derive(Debug, Clone)]
pub struct AtlasLayout {
    pub dimensions: (u32, u32),
    pub frames_per_row: u32,
    pub rows: u32,
    pub frame_size: (u32, u32),
}

/// Prepares analysed frames for placement into the final atlas grid.
///
/// This involves creating a canvas of the final `frame_width` x `frame_height`
/// for each frame and positioning the actual sprite content within it,
/// centred around the standard reference point and adjusted by the
/// original animation offsets.
pub fn prepare_frames(
    analysis: &mut FrameAnalysis, // Changed to mutable
    frame_width: u32,
    frame_height: u32,
) -> Result<Vec<RgbaImage>, super::AtlasError> {
    let mut prepared_frames = Vec::with_capacity(analysis.total_original_frames);

    for (_idx, (_anim_id, _dir_idx, _sequence_idx, analysed_frame)) in
        analysis.ordered_frames.iter_mut().enumerate()
    {
        let mut final_frame_canvas = RgbaImage::new(frame_width, frame_height);
        let (content_width, content_height) = analysed_frame.image.dimensions();

        if content_width == 0 || content_height == 0 {
            analysed_frame.final_placement_x = 0;
            analysed_frame.final_placement_y = 0;
            prepared_frames.push(final_frame_canvas);
            continue;
        }

        // Position the cropped image so its reference point aligns with the frame reference point
        let final_pos_x = (frame_width as i32 - content_width as i32) / 2;
        let final_pos_y = (frame_height as i32 - content_height as i32) / 2;

        // Store the calculated position for use in metadata generation
        analysed_frame.final_placement_x = final_pos_x;
        analysed_frame.final_placement_y = final_pos_y;

        // Overlay the  content image onto the canvas using the corrected position
        overlay_image(
            &mut final_frame_canvas,
            &analysed_frame.image,
            final_pos_x,
            final_pos_y,
        );

        prepared_frames.push(final_frame_canvas);
    }

    Ok(prepared_frames)
}

/// Creates an atlas layout grid based on the number of frames and frame size.
pub fn create_atlas_layout(
    total_unique_frames: usize,
    frame_width: u32,
    frame_height: u32,
) -> AtlasLayout {
    if total_unique_frames == 0 {
        return AtlasLayout {
            dimensions: (frame_width.max(8), frame_height.max(8)),
            frames_per_row: 1,
            rows: 1,
            frame_size: (frame_width, frame_height),
        };
    }

    // Calculate a near-square grid layout
    let frames_per_row = (total_unique_frames as f32).sqrt().ceil() as u32;
    let rows = (total_unique_frames as u32 + frames_per_row - 1) / frames_per_row;

    // Calculate atlas dimensions
    let atlas_width = frames_per_row * frame_width;
    let atlas_height = rows * frame_height;

    AtlasLayout {
        dimensions: (atlas_width, atlas_height),
        frames_per_row,
        rows,
        frame_size: (frame_width, frame_height),
    }
}

/// Generates the final atlas image by placing unique frames according to the layout.
pub fn generate_atlas(
    unique_frames: &[RgbaImage],
    layout: &AtlasLayout,
) -> Result<RgbaImage, super::AtlasError> {
    if unique_frames.is_empty() {
        return Err(super::AtlasError::NoFramesFound);
    }
    let (atlas_width, atlas_height) = layout.dimensions;
    let (frame_width, frame_height) = layout.frame_size;

    let mut atlas = RgbaImage::new(atlas_width, atlas_height);

    // Place unique frames onto the atlas
    for (i, frame) in unique_frames.iter().enumerate() {
        // Ensure frame matches expected layout size
        if frame.width() != frame_width || frame.height() != frame_height {
            eprintln!(
                "Warning: Frame {} has dimensions {}x{}, expected {}x{}. Skipping placement.",
                i,
                frame.width(),
                frame.height(),
                frame_width,
                frame_height
            );
            continue;
        }

        let atlas_col = i as u32 % layout.frames_per_row;
        let atlas_row = i as u32 / layout.frames_per_row;
        let x = atlas_col * frame_width;
        let y = atlas_row * frame_height;

        overlay_image(&mut atlas, frame, x as i32, y as i32);
    }

    Ok(atlas)
}

/// Deduplicates frames by comparing pixel data using xxHash
///
/// Returns a tuple: `(Vec<RgbaImage>, Vec<usize>)` where the first element
/// is the vector of unique frames, and the second is a mapping vector where
/// `mapping[original_index] = unique_index`.
pub fn deduplicate_frames(frames: &[RgbaImage]) -> (Vec<RgbaImage>, Vec<usize>) {
    let mut unique_frames_map: HashMap<u64, usize> = HashMap::new();
    let mut unique_frames_vec = Vec::new();
    let mut frame_mapping = Vec::with_capacity(frames.len());

    for frame in frames {
        let frame_hash = calculate_frame_hash(frame);

        let unique_index = match unique_frames_map.entry(frame_hash) {
            Entry::Occupied(entry) => {
                let candidate_idx = *entry.get();
                // Verify to handle hash collisions
                if frames_are_identical(frame, &unique_frames_vec[candidate_idx]) {
                    candidate_idx
                } else {
                    // Hash collision - still a unique frame
                    let new_idx = unique_frames_vec.len();
                    unique_frames_vec.push(frame.clone());
                    unique_frames_map.insert(frame_hash, new_idx);
                    new_idx
                }
            }
            Entry::Vacant(entry) => {
                let index = unique_frames_vec.len();
                unique_frames_vec.push(frame.clone());
                entry.insert(index);
                index
            }
        };

        frame_mapping.push(unique_index);
    }

    (unique_frames_vec, frame_mapping)
}

/// Calculate a 64-bit hash of an image frame for fast comparison
fn calculate_frame_hash(frame: &RgbaImage) -> u64 {
    let mut hasher = XxHash64::default();
    frame.as_raw().hash(&mut hasher);
    hasher.finish()
}

/// Check if two frames are pixel-for-pixel identical
/// Only used when hash values match to confirm there's no collision
fn frames_are_identical(a: &RgbaImage, b: &RgbaImage) -> bool {
    if a.dimensions() != b.dimensions() {
        return false;
    }
    a.as_raw() == b.as_raw()
}

/// Overlays a smaller image onto a larger canvas image at specified coordinates.
fn overlay_image(canvas: &mut RgbaImage, image: &RgbaImage, x: i32, y: i32) {
    imageops::overlay(canvas, image, x as i64, y as i64);
}
