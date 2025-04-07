//! Renderer for WAN sprite frames
//!
//! This module provides functionality to render individual frames from WAN files
//! into RGBA images, handling position offsets, flipping, and palette mapping.

use image::{Rgba, RgbaImage};

use super::model::{FrameOffset, MetaFramePiece, WanFile};
use super::{WanError, WanType, TEX_SIZE};

/// Extract a single frame from a WAN file
pub fn extract_frame(wan: &WanFile, frame_idx: usize) -> Result<RgbaImage, WanError> {
    if frame_idx >= wan.frame_data.len() {
        return Err(WanError::OutOfBounds(format!(
            "Frame index {} out of bounds (max: {})",
            frame_idx,
            wan.frame_data.len() - 1
        )));
    }

    // Get the frame data
    let frame = &wan.frame_data[frame_idx];

    // Calculate the frame bounds
    let bounds = get_frame_bounds(wan, frame_idx)?;

    // Calculate frame dimensions
    let width = bounds.2 - bounds.0;
    let height = bounds.3 - bounds.1;

    // Ensure minimum size
    let width = width.max(8);
    let height = height.max(8);

    // Create output image with calculated size
    let mut image = RgbaImage::new(width as u32, height as u32);

    // Track rendered pieces for debugging
    let total_pieces = frame.pieces.len();
    let mut rendered_pieces = 0;
    let mut minus_frame_pieces = 0;
    let mut resolved_references = 0;
    let mut failed_references = 0;

    // Render each piece of the frame
    for (piece_idx, piece) in frame.pieces.iter().enumerate() {
        // Handle MINUS_FRAME references - using SkyTemple's exact algorithm
        let actual_img_idx = if piece.img_index < 0 {
            minus_frame_pieces += 1;
            println!("Processing MINUS_FRAME reference in piece {} with tile num {}", 
                     piece_idx, piece.get_tile_num());
            
            // Get the target tile number to match
            let target_tile_num = piece.get_tile_num();
            
            // Start from the current piece and search backward (SkyTemple style)
            let mut reference_img_idx = None;
            let mut prev_idx = piece_idx;
            
            // Use a while loop for precise traversal control
            while prev_idx > 0 {
                prev_idx -= 1;
                let prev_piece = &frame.pieces[prev_idx];
                
                // CRITICAL: Exactly match SkyTemple's logic - skip if EITHER condition is true
                // (This is different from the original logic that required BOTH conditions)
                if prev_piece.img_index < 0 || prev_piece.get_tile_num() != target_tile_num {
                    // Skip this piece and continue searching
                    continue;
                }
                
                // Found a valid reference with matching tile number and valid img_index
                reference_img_idx = Some(prev_piece.img_index as usize);
                println!("  Found reference in piece {} with img_index {}", 
                         prev_idx, prev_piece.img_index);
                break;
            }
            
            match reference_img_idx {
                Some(idx) => {
                    resolved_references += 1;
                    idx
                },
                None => {
                    failed_references += 1;
                    println!("  WARNING: No reference found for MINUS_FRAME piece {} with tile num {}",
                             piece_idx, piece.get_tile_num());
                    continue;  // Skip this piece as we can't resolve its reference
                }
            }
        } else {
            piece.img_index as usize
        };

        // Bounds check image index
        if actual_img_idx >= wan.img_data.len() {
            println!(
                "WARNING: Image index {} for piece {} out of bounds (max: {})",
                actual_img_idx,
                piece_idx,
                wan.img_data.len() - 1
            );
            continue;
        }

        // Get piece dimensions
        let dimensions = piece.get_dimensions();

        // Get piece position relative to bounds
        let pos_x = piece.get_x_offset() - bounds.0;
        let pos_y = piece.get_y_offset() - bounds.1;

        // Get palette
        let pal_num = piece.get_pal_num() as usize;
        if pal_num >= wan.custom_palette.len() {
            println!(
                "WARNING: Palette {} for piece {} out of bounds (max: {})",
                pal_num,
                piece_idx,
                wan.custom_palette.len() - 1
            );
            continue;
        }

        let palette = &wan.custom_palette[pal_num];

        // Render the piece
        match render_piece(
            wan,
            actual_img_idx,
            piece,
            &mut image,
            (pos_x as i32, pos_y as i32),
            (dimensions.0 * TEX_SIZE, dimensions.1 * TEX_SIZE),
            palette,
        ) {
            Ok(_) => {
                rendered_pieces += 1;
            }
            Err(e) => {
                println!("ERROR rendering piece {}: {:?}", piece_idx, e);
            }
        }
    }

    // Print detailed summary for debugging
    let non_transparent_count = image.pixels().filter(|p| p[3] > 0).count();
    println!(
        "Frame {} summary: {} pieces total, {} rendered, {} MINUS_FRAME refs ({} resolved, {} failed), {} visible pixels",
        frame_idx, total_pieces, rendered_pieces, minus_frame_pieces, 
        resolved_references, failed_references, non_transparent_count
    );

    Ok(image)
}

/// Extract and render all frames from a specific animation in the WAN file
pub fn extract_animation_frames(
    wan: &WanFile,
    anim_group_idx: usize,
    anim_idx: usize,
    dir_idx: usize,
) -> Result<Vec<RgbaImage>, WanError> {
    // Check animation group bounds
    if anim_group_idx >= wan.animation_groups.len() {
        return Err(WanError::OutOfBounds(format!(
            "Animation group index {} out of bounds (max: {})",
            anim_group_idx,
            wan.animation_groups.len() - 1
        )));
    }

    let anim_group = &wan.animation_groups[anim_group_idx];

    // Check animation bounds
    if anim_idx >= anim_group.len() {
        return Err(WanError::OutOfBounds(format!(
            "Animation index {} out of bounds (max: {})",
            anim_idx,
            anim_group.len() - 1
        )));
    }

    let animation = &anim_group[anim_idx];

    // Check if animation is empty
    if animation.frames.is_empty() {
        return Ok(Vec::new());
    }

    // Extract each frame
    let mut frames = Vec::with_capacity(animation.frames.len());

    for frame in &animation.frames {
        let frame_idx = frame.frame_index as usize;
        if frame_idx >= wan.frame_data.len() {
            return Err(WanError::OutOfBounds(format!(
                "Frame index {} referenced by animation is out of bounds (max: {})",
                frame_idx,
                wan.frame_data.len() - 1
            )));
        }

        let frame_img = extract_frame(wan, frame_idx)?;
        frames.push(frame_img);
    }

    Ok(frames)
}

/// Render a frame to the provided image at the specified offset
fn render_frame(
    wan: &WanFile,
    frame_idx: usize,
    image: &mut RgbaImage,
    bounds: (i16, i16, i16, i16),
) -> Result<(), WanError> {
    let frame = &wan.frame_data[frame_idx];

    // Track stats for debugging
    let total_pieces = frame.pieces.len();
    let mut rendered_pieces = 0;
    let mut minus_frame_pieces = 0;
    let mut resolved_references = 0;
    let mut failed_references = 0;

    // Render each piece of the frame
    for (piece_idx, piece) in frame.pieces.iter().enumerate() {
        // Get the actual image index to use, resolving MINUS_FRAME references
        let actual_img_idx = if piece.img_index < 0 {
            minus_frame_pieces += 1;
            // Using SkyTemple's exact algorithm for MINUS_FRAME resolution
            println!("Processing MINUS_FRAME reference in piece {} with tile num {}", 
                     piece_idx, piece.get_tile_num());
            
            // Get the target tile number to match
            let target_tile_num = piece.get_tile_num();
            
            // Start from the current piece and search backward
            let mut reference_img_idx = None;
            let mut prev_idx = piece_idx;
            
            // Use while loop for precise control (SkyTemple style)
            while prev_idx > 0 {
                prev_idx -= 1;
                let prev_piece = &frame.pieces[prev_idx];
                
                // CRITICAL CHANGE: Skip pieces that are EITHER:
                // 1. MINUS_FRAME themselves OR
                // 2. Don't have the matching tile number
                if prev_piece.img_index < 0 || prev_piece.get_tile_num() != target_tile_num {
                    continue; // Skip this piece and continue searching
                }
                
                // Found a valid reference
                reference_img_idx = Some(prev_piece.img_index as usize);
                println!("  Found reference in piece {} with img_index {}", 
                         prev_idx, prev_piece.img_index);
                break;
            }
            
            match reference_img_idx {
                Some(idx) => {
                    resolved_references += 1;
                    idx
                },
                None => {
                    failed_references += 1;
                    println!("  WARNING: No reference found for MINUS_FRAME piece {} with tile num {}",
                             piece_idx, piece.get_tile_num());
                    continue; // Skip this piece as we can't resolve its reference
                }
            }
        } else {
            // Direct image reference
            piece.img_index as usize
        };

        // Bounds check image index
        if actual_img_idx >= wan.img_data.len() {
            return Err(WanError::OutOfBounds(format!(
                "Image index {} out of bounds (max: {})",
                actual_img_idx,
                wan.img_data.len() - 1
            )));
        }

        // Get image piece dimensions
        let dimensions = piece.get_dimensions();

        // Get piece position relative to bounds
        let pos_x = piece.get_x_offset() - bounds.0;
        let pos_y = piece.get_y_offset() - bounds.1;

        // Get palette number
        let pal_num = piece.get_pal_num() as usize;
        if pal_num >= wan.custom_palette.len() {
            return Err(WanError::OutOfBounds(format!(
                "Palette number {} out of bounds (max: {})",
                pal_num,
                wan.custom_palette.len() - 1
            )));
        }
        let palette = &wan.custom_palette[pal_num];

        // Render the piece
        match render_piece(
            wan,
            actual_img_idx,
            piece,
            image,
            (pos_x as i32, pos_y as i32),
            (
                dimensions.0 * super::TEX_SIZE,
                dimensions.1 * super::TEX_SIZE,
            ),
            palette,
        ) {
            Ok(_) => {
                rendered_pieces += 1;
            }
            Err(e) => {
                println!("  Error rendering piece {}: {:?}", piece_idx, e);
            }
        }
    }

    // Print a summary of the frame rendering for debugging
    println!(
        "Frame {} render summary: {} pieces total, {} rendered, {} MINUS_FRAME refs ({} resolved, {} failed)",
        frame_idx, total_pieces, rendered_pieces, minus_frame_pieces, 
        resolved_references, failed_references
    );

    Ok(())
}

/// Render an individual piece of a frame to the image
fn render_piece(
    wan: &WanFile,
    img_idx: usize,
    piece: &MetaFramePiece,
    image: &mut RgbaImage,
    pos: (i32, i32),
    dimensions: (usize, usize),
    palette: &[(u8, u8, u8, u8)],
) -> Result<(), WanError> {
    // Validate inputs
    if img_idx >= wan.img_data.len() {
        return Err(WanError::OutOfBounds(format!(
            "Image index {} out of bounds (max: {})",
            img_idx,
            wan.img_data.len() - 1
        )));
    }

    let (width, height) = dimensions;
    if width == 0 || height == 0 || palette.is_empty() {
        return Err(WanError::InvalidData(
            "Invalid dimensions or empty palette".to_string(),
        ));
    }

    // Get the image piece data
    let img_piece = &wan.img_data[img_idx];

    // Step 1: Create a temporary image for the piece (before any flipping)
    let mut piece_img = RgbaImage::new(width as u32, height as u32);

    // Track if any non-transparent pixels were drawn
    let mut has_visible_pixels = false;

    // Check if using 256-color mode
    let is_256_color = piece.is_color_pal_256();

    // Process image data according to color mode
    if is_256_color {
        // 256-color (8bpp) mode
        // SkyTemple uses a special offset calculation for 256-color mode
        let block_offset = piece.get_tile_num() as usize;
        let byte_pos = block_offset * 2 * TEX_SIZE * TEX_SIZE;

        // Find the right image data chunk
        let mut flat_img_px = Vec::new();
        let mut cur_byte = 0;

        for strip in &img_piece.img_px {
            if cur_byte == byte_pos {
                // Found the right position, use this data
                for &px in strip {
                    flat_img_px.push(px);
                }
                break;
            }

            // Move counter forward
            cur_byte += strip.len();

            // Align to blocks of 128 bytes (SkyTemple's approach)
            while cur_byte % (2 * TEX_SIZE * TEX_SIZE) != 0 {
                cur_byte += 64;
            }
        }

        // Render the image from the flattened data
        for yy in 0..height / TEX_SIZE {
            for xx in 0..width / TEX_SIZE {
                let block_idx = yy * (width / TEX_SIZE) + xx;
                let tex_position = block_idx * TEX_SIZE * TEX_SIZE;

                if tex_position < flat_img_px.len() {
                    for py in 0..TEX_SIZE {
                        for px in 0..TEX_SIZE {
                            let pixel_idx = tex_position + py * TEX_SIZE + px;

                            if pixel_idx >= flat_img_px.len() {
                                continue;
                            }

                            let palette_idx = flat_img_px[pixel_idx] as usize;
                            if palette_idx == 0 || palette_idx >= palette.len() {
                                continue; // Skip transparent or invalid palette indices
                            }

                            let color = palette[palette_idx];
                            let pixel_x = (xx * TEX_SIZE + px) as u32;
                            let pixel_y = (yy * TEX_SIZE + py) as u32;

                            piece_img.put_pixel(
                                pixel_x,
                                pixel_y,
                                Rgba([color.0, color.1, color.2, color.3]),
                            );

                            if color.3 > 0 {
                                has_visible_pixels = true;
                            }
                        }
                    }
                }
            }
        }
    } else {
        // 16-color (4bpp) mode - SkyTemple's approach
        // First, flatten all pixel data and unpack 4bpp pixels here
        let mut flat_img_px = Vec::new();

        // Unpack the 4bpp pixels during rendering
        for strip in &img_piece.img_px {
            for &px in strip {
                // Each byte represents TWO 4-bit pixels
                flat_img_px.push(px % 16); // Lower 4 bits (first pixel)
                flat_img_px.push(px / 16); // Upper 4 bits (second pixel)
            }
        }

        // Calculate blocks in width and height
        let blocks_width = width / TEX_SIZE;
        let blocks_height = height / TEX_SIZE;

        // Render the image
        for yy in 0..blocks_height {
            for xx in 0..blocks_width {
                let block_idx = yy * blocks_width + xx;
                let tex_position = block_idx * TEX_SIZE * TEX_SIZE;

                for py in 0..TEX_SIZE {
                    for px in 0..TEX_SIZE {
                        let pixel_idx = tex_position + py * TEX_SIZE + px;

                        if pixel_idx >= flat_img_px.len() {
                            continue;
                        }

                        let palette_idx = flat_img_px[pixel_idx] as usize;
                        if palette_idx == 0 || palette_idx >= palette.len() {
                            continue; // Skip transparent or invalid palette indices
                        }

                        let color = palette[palette_idx];
                        let pixel_x = (xx * TEX_SIZE + px) as u32;
                        let pixel_y = (yy * TEX_SIZE + py) as u32;

                        piece_img.put_pixel(
                            pixel_x,
                            pixel_y,
                            Rgba([color.0, color.1, color.2, color.3]),
                        );

                        if color.3 > 0 {
                            has_visible_pixels = true;
                        }
                    }
                }
            }
        }
    }

    // If the texture is completely transparent, log and return early
    if !has_visible_pixels {
        println!(
            "WARNING: No visible pixels rendered for piece at index {}",
            img_idx
        );
        return Ok(());
    }

    // Step 3: Apply flipping to the entire image - SkyTemple way
    if piece.is_h_flip() {
        piece_img = image::imageops::flip_horizontal(&piece_img);
    }

    if piece.is_v_flip() {
        piece_img = image::imageops::flip_vertical(&piece_img);
    }

    // Step 4: Paste the piece onto the destination image
    for (x, y, pixel) in piece_img.enumerate_pixels() {
        // Skip transparent pixels
        if pixel[3] == 0 {
            continue;
        }

        let dest_x = pos.0 + x as i32;
        let dest_y = pos.1 + y as i32;

        // Only draw within bounds
        if dest_x >= 0
            && dest_y >= 0
            && dest_x < image.width() as i32
            && dest_y < image.height() as i32
        {
            image.put_pixel(dest_x as u32, dest_y as u32, *pixel);
        }
    }

    Ok(())
}

/// Get the bounds of a frame
pub fn get_frame_bounds(wan: &WanFile, frame_idx: usize) -> Result<(i16, i16, i16, i16), WanError> {
    let frame = &wan.frame_data[frame_idx];
    let mut bounds = (i16::MAX, i16::MAX, i16::MIN, i16::MIN);

    // Check if frame is empty
    if frame.pieces.is_empty() {
        return Ok((0, 0, 0, 0));
    }

    // Calculate bounds based on all pieces
    for piece in &frame.pieces {
        let piece_bounds = piece.get_bounds();

        bounds.0 = bounds.0.min(piece_bounds.0);
        bounds.1 = bounds.1.min(piece_bounds.1);
        bounds.2 = bounds.2.max(piece_bounds.2);
        bounds.3 = bounds.3.max(piece_bounds.3);
    }

    // Add body part offsets to bounds
    if frame_idx < wan.offset_data.len() {
        let offset = &wan.offset_data[frame_idx];
        let offset_bounds = offset.get_bounds();

        bounds.0 = bounds.0.min(offset_bounds.0);
        bounds.1 = bounds.1.min(offset_bounds.1);
        bounds.2 = bounds.2.max(offset_bounds.2);
        bounds.3 = bounds.3.max(offset_bounds.3);
    }

    // Ensure minimum size
    if bounds.2 <= bounds.0 {
        bounds.2 = bounds.0 + 1;
    }
    if bounds.3 <= bounds.1 {
        bounds.3 = bounds.1 + 1;
    }

    Ok(bounds)
}
