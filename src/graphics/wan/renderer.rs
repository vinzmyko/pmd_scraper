//! Renderer for WAN sprite frames
//!
//! This module provides functionality to render individual frames from WAN files
//! into RGBA images, handling position offsets, flipping, and palette mapping.

use crate::graphics::wan::{
    model::{MetaFramePiece, WanFile},
    WanError, TEX_SIZE,
};

use image::{Rgba, RgbaImage};

/// Extract a single frame from a WAN file
pub fn extract_frame(wan: &WanFile, frame_idx: usize) -> Result<RgbaImage, WanError> {
    if frame_idx >= wan.frame_data.len() {
        return Err(WanError::OutOfBounds(format!(
            "Frame index {} out of bounds (max: {})",
            frame_idx,
            wan.frame_data.len() - 1
        )));
    }

    let frame_data = &wan.frame_data[frame_idx];

    let frame_bounds = get_frame_bounds(wan, frame_idx)?;

    // Calculate frame dimensions
    let width = frame_bounds.2 - frame_bounds.0;
    let height = frame_bounds.3 - frame_bounds.1;

    // Ensure minimum size
    let width = width.max(8);
    let height = height.max(8);

    // Create output image with calculated size
    let mut image = RgbaImage::new(width as u32, height as u32);

    // Render each piece of the frame
    for (piece_idx, piece) in frame_data.pieces.iter().enumerate() {
        let actual_img_idx = if piece.img_index < 0 {
            // Get the target tile number to match
            let target_tile_num = piece.get_tile_num();

            // Start from the current piece and search backward
            let mut reference_img_idx = None;
            let mut prev_idx = piece_idx;

            while prev_idx > 0 {
                prev_idx -= 1;
                let prev_piece = &frame_data.pieces[prev_idx];

                if prev_piece.img_index < 0 || prev_piece.get_tile_num() != target_tile_num {
                    // Skip this piece and continue searching
                    continue;
                }

                // Found a valid reference with matching tile number and valid img_index
                reference_img_idx = Some(prev_piece.img_index as usize);
                break;
            }

            match reference_img_idx {
                Some(idx) => idx,
                None => {
                    continue;
                }
            }
        } else {
            piece.img_index as usize
        };

        // Bounds check image index
        if actual_img_idx >= wan.img_data.len() {
            continue;
        }

        // Get piece dimensions
        let dimensions = piece.get_dimensions();

        // Get piece position relative to bounds
        let pos_x = piece.get_x_offset() - frame_bounds.0;
        let pos_y = piece.get_y_offset() - frame_bounds.1;

        let pal_num = piece.get_pal_num() as usize;
        if pal_num >= wan.custom_palette.len() {
            continue;
        }

        let palette = &wan.custom_palette[pal_num];

        match render_piece(
            wan,
            actual_img_idx,
            piece,
            &mut image,
            (pos_x as i32, pos_y as i32),
            (dimensions.0 * TEX_SIZE, dimensions.1 * TEX_SIZE),
            palette,
        ) {
            Ok(_) => {}
            Err(e) => {
                println!("ERROR rendering piece {}: {:?}", piece_idx, e);
            }
        }
    }

    Ok(image)
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
        return Err(WanError::InvalidDataStructure(
            "Invalid dimensions or empty palette".to_string(),
        ));
    }

    let img_piece = &wan.img_data[img_idx];

    // Create a temporary image for the piece (before any flipping)
    let mut piece_img = RgbaImage::new(width as u32, height as u32);

    // Track if any non-transparent pixels were drawn
    let mut has_visible_pixels = false;

    // Check if using 256-colour mode
    let is_256_colour = piece.is_colour_pal_256();

    // Process image data according to colour mode
    if is_256_colour {
        // 256-colour (8bpp) mode
        let block_offset = piece.get_tile_num() as usize;
        let byte_pos = block_offset * 2 * TEX_SIZE * TEX_SIZE;

        // Find the right image data chunk
        let mut flat_img_px = Vec::new();
        let mut cur_byte = 0;

        for strip in &img_piece.img_px {
            if cur_byte == byte_pos {
                for &px in strip {
                    flat_img_px.push(px);
                }
                break;
            }

            // Move counter forward
            cur_byte += strip.len();

            // Align to blocks of 128 bytes
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
                                continue;
                            }

                            let colour = palette[palette_idx];
                            let pixel_x = (xx * TEX_SIZE + px) as u32;
                            let pixel_y = (yy * TEX_SIZE + py) as u32;

                            piece_img.put_pixel(
                                pixel_x,
                                pixel_y,
                                Rgba([colour.0, colour.1, colour.2, colour.3]),
                            );

                            if colour.3 > 0 {
                                has_visible_pixels = true;
                            }
                        }
                    }
                }
            }
        }
    } else {
        // 16-colour (4bpp) mode
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

                        let colour = palette[palette_idx];
                        let pixel_x = (xx * TEX_SIZE + px) as u32;
                        let pixel_y = (yy * TEX_SIZE + py) as u32;

                        piece_img.put_pixel(
                            pixel_x,
                            pixel_y,
                            Rgba([colour.0, colour.1, colour.2, colour.3]),
                        );

                        if colour.3 > 0 {
                            has_visible_pixels = true;
                        }
                    }
                }
            }
        }
    }

    if !has_visible_pixels {
        return Ok(());
    }

    // Apply flipping to the entire image
    if piece.is_h_flip() {
        piece_img = image::imageops::flip_horizontal(&piece_img);
    }

    if piece.is_v_flip() {
        piece_img = image::imageops::flip_vertical(&piece_img);
    }

    // Paste the piece onto the destination image
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
    if frame_idx < wan.body_part_offset_data.len() {
        let offset = &wan.body_part_offset_data[frame_idx];
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
