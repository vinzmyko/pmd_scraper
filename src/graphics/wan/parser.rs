//! Parser for WAN sprite format
//!
//! This module provides functions to parse WAN files from binary data,
//! supporting both character and effect WAN variants.

use std::collections::HashMap;
use std::io::{self, Cursor, Read, Seek, SeekFrom};

use super::model::{
    Animation, FrameOffset, ImgPiece, MetaFrame, MetaFramePiece, SequenceFrame, WanFile,
};
use super::{WanError, WanType};

use crate::containers::sir0;

pub fn read_u8(reader: &mut Cursor<&[u8]>) -> io::Result<u8> {
    if reader.position() >= reader.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "End of buffer reached",
        ));
    }

    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf)?;
    Ok(buf[0])
}

pub fn read_u16_le(reader: &mut Cursor<&[u8]>) -> io::Result<u16> {
    if reader.position() + 1 >= reader.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "End of buffer reached or not enough bytes for u16",
        ));
    }

    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

pub fn read_u32_le(reader: &mut Cursor<&[u8]>) -> io::Result<u32> {
    if reader.position() + 3 >= reader.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "End of buffer reached or not enough bytes for u32",
        ));
    }

    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

/// Read an i16 in little-endian format from the cursor
pub fn read_i16_le(reader: &mut Cursor<&[u8]>) -> Result<i16, io::Error> {
    if reader.position() + 1 >= reader.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "End of buffer reached or not enough bytes for i16",
        ));
    }

    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    Ok(i16::from_le_bytes(buf))
}

/// Parse a WAN file from binary data
pub fn parse_wan(data: &[u8], wan_type: WanType) -> Result<WanFile, WanError> {
    // First unwrap SIR0 container
    let sir0_data = sir0::Sir0::from_bytes(data)
        .map_err(|e| WanError::Sir0Error(format!("Failed to parse SIR0: {}", e)))?;

    // Create a cursor for reading the content
    let mut reader = Cursor::new(sir0_data.content.as_slice());
    let buffer_size = sir0_data.content.len() as u64;

    // Read from the data pointer position
    reader
        .seek(SeekFrom::Start(sir0_data.data_pointer as u64))
        .map_err(|e| WanError::Io(e))?;

    // Parse based on WAN type - pass the pointer offsets
    match wan_type {
        WanType::Character => {
            parse_character_wan(&mut reader, buffer_size)
        }
        WanType::Effect => {
            parse_effect_wan(&mut reader, buffer_size)
        }
    }
}

/// Auto-detect and parse a WAN file from SIR0 content
pub fn auto_detect_and_parse_wan(data: &[u8], data_pointer: u32) -> Result<WanFile, WanError> {
    // Parse the SIR0 container to get its content and pointer offset table
    let sir0_data = match sir0::Sir0::from_bytes(data) {
        Ok(sir0) => {
            println!("  - Successfully parsed SIR0 container");
            println!(
                "  - Found {} pointer offsets for resolution",
                sir0.content_pointer_offsets.len()
            );
            sir0
        }
        Err(e) => {
            // If SIR0 parsing fails, we'll continue with no pointer offsets
            // This preserves backward compatibility but logs warnings
            println!("  - Warning: SIR0 parsing failed: {}", e);
            println!("  - Continuing with direct data access and no pointer offsets");

            // Create cursor directly on the input data
            let mut reader = Cursor::new(data);
            let buffer_size = data.len() as u64;

            // Seek to data_pointer position
            if let Err(e) = reader.seek(SeekFrom::Start(data_pointer as u64)) {
                return Err(WanError::Io(e));
            }

            return detect_and_parse_wan_type(
                &mut reader,
                buffer_size,
                data_pointer,
            );
        }
    };

    // Create a cursor for the SIR0 content
    let mut reader = Cursor::new(sir0_data.content.as_slice());
    let buffer_size = sir0_data.content.len() as u64;

    // Use the data_pointer and pointer offsets from the SIR0 container
    let wan_data_pointer = sir0_data.data_pointer;

    // Seek to the WAN header position
    reader
        .seek(SeekFrom::Start(wan_data_pointer as u64))
        .map_err(|e| WanError::Io(e))?;

    // Pass the pointer offsets to the WAN parser
    detect_and_parse_wan_type(
        &mut reader,
        buffer_size,
        wan_data_pointer,
    )
}

/// Helper function to detect WAN type and call appropriate parser
fn detect_and_parse_wan_type(
    reader: &mut Cursor<&[u8]>,
    buffer_size: u64,
    data_pointer: u32,
) -> Result<WanFile, WanError> {
    // Read the first 8 bytes to identify the WAN type
    let ptr_anim_info = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
    let ptr_image_data_info = read_u32_le(reader).map_err(|e| WanError::Io(e))?;

    // Check for valid pointers
    if ptr_anim_info == 0 || ptr_image_data_info == 0 {
        return Err(WanError::InvalidData(
            "Null pointer in WAN header".to_string(),
        ));
    }

    // Read the image type
    let img_type = read_u16_le(reader).map_err(|e| WanError::Io(e))?;

    // Reset cursor position
    reader
        .seek(SeekFrom::Start(data_pointer as u64))
        .map_err(|e| WanError::Io(e))?;

    // Based on img_type, choose the correct parser
    match img_type {
        1 => {
            println!("  - Detected Character WAN (imgType=1)");
            parse_character_wan(reader, buffer_size)
        }
        2 | 3 => {
            println!("  - Detected Effect WAN (imgType={})", img_type);
            parse_effect_wan(reader, buffer_size)
        }
        _ => {
            // If unknown type, try Character first, then Effect as fallback
            println!(
                "  - Unknown WAN type (imgType={}), trying Character parser",
                img_type
            );
            match parse_character_wan(reader, buffer_size) {
                Ok(wan) => Ok(wan),
                Err(e) => {
                    println!("  - Character parser failed: {:?}, trying Effect parser", e);
                    // Reset cursor position for effect parser
                    reader
                        .seek(SeekFrom::Start(data_pointer as u64))
                        .map_err(|e| WanError::Io(e))?;

                    parse_effect_wan(reader, buffer_size)
                }
            }
        }
    }
}

/// Parse WAN file from SIR0 content that has already been extracted
pub fn parse_wan_from_sir0_content(
    content: &[u8],
    data_pointer: u32,
    wan_type: WanType,
) -> Result<WanFile, WanError> {
    let mut reader = Cursor::new(content);
    let buffer_size = content.len() as u64;

    // Read from the data pointer position
    reader
        .seek(SeekFrom::Start(data_pointer as u64))
        .map_err(|e| WanError::Io(e))?;

    // Parse based on WAN type, passing pointer offsets
    match wan_type {
        WanType::Character => parse_character_wan(&mut reader, buffer_size),
        WanType::Effect => parse_effect_wan(&mut reader, buffer_size),
    }
}

/// Parse a character WAN file (from monster.bin, etc.)
pub fn parse_character_wan(
    reader: &mut Cursor<&[u8]>,
    buffer_size: u64,
) -> Result<WanFile, WanError> {
    println!("Beginning WAN parsing");

    // Store current position to check for minimal header
    let start_pos = reader.position();

    // Make sure we have enough bytes for the basic header
    if start_pos + 8 > buffer_size {
        return Err(WanError::InvalidData(format!(
            "Not enough bytes for WAN header. Position: {}, Buffer size: {}",
            start_pos, buffer_size
        )));
    }

    // Read WAN header
    let ptr_anim_info = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
    let ptr_image_data_info = read_u32_le(reader).map_err(|e| WanError::Io(e))?;

    // Validate pointers
    if ptr_anim_info == 0 || ptr_image_data_info == 0 {
        return Err(WanError::InvalidData(
            "Null pointer in WAN header".to_string(),
        ));
    }

    // Check if pointers are within bounds
    if ptr_anim_info as u64 >= buffer_size || ptr_image_data_info as u64 >= buffer_size {
        return Err(WanError::InvalidData(format!(
            "Pointer out of bounds. anim_info: {:#x}, image_data_info: {:#x}, buffer size: {}",
            ptr_anim_info, ptr_image_data_info, buffer_size
        )));
    }

    // Read image type (should be 1 for character sprites)
    let img_type = read_u16_le(reader).map_err(|e| WanError::Io(e))?;
    if img_type != 1 {
        return Err(WanError::InvalidData(format!(
            "Expected image type 1 for character sprite, got {}",
            img_type
        )));
    }

    // Skip unknown value
    read_u16_le(reader).map_err(|e| WanError::Io(e))?;

    // Read image data info with bounds checking
    if ptr_image_data_info as u64 >= buffer_size {
        return Err(WanError::InvalidData(format!(
            "Image data info pointer out of bounds: {:#x}, buffer size: {}",
            ptr_image_data_info, buffer_size
        )));
    }

    reader
        .seek(SeekFrom::Start(ptr_image_data_info as u64))
        .map_err(|e| WanError::Io(e))?;

    let ptr_image_data_table = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
    let ptr_palette_info = read_u32_le(reader).map_err(|e| WanError::Io(e))?;

    // Validate pointers
    if ptr_image_data_table == 0 || ptr_palette_info == 0 {
        return Err(WanError::InvalidData(
            "Null pointer in Image Data Info".to_string(),
        ));
    }

    // Bounds check these pointers too
    if ptr_image_data_table as u64 >= buffer_size || ptr_palette_info as u64 >= buffer_size {
        return Err(WanError::InvalidData(format!(
            "Pointer out of bounds. image_data_table: {:#x}, palette_info: {:#x}, buffer size: {}",
            ptr_image_data_table, ptr_palette_info, buffer_size
        )));
    }

    // Skip unknown values (Unk#13, Is256ColorSpr, Unk#11)
    read_u16_le(reader).map_err(|e| WanError::Io(e))?; // Unk#13 - ALWAYS 0
    read_u16_le(reader).map_err(|e| WanError::Io(e))?; // Is256ColorSpr - ALWAYS 0
    read_u16_le(reader).map_err(|e| WanError::Io(e))?; // Unk#11 - ALWAYS 1 unless empty

    // Read number of images
    let nb_imgs = read_u16_le(reader).map_err(|e| WanError::Io(e))?;

    // Read palette info with bounds checking
    reader
        .seek(SeekFrom::Start(ptr_palette_info as u64))
        .map_err(|e| WanError::Io(e))?;

    let ptr_palette_data_block = read_u32_le(reader).map_err(|e| WanError::Io(e))?;

    // Bounds check palette data block
    if ptr_palette_data_block as u64 >= buffer_size {
        return Err(WanError::InvalidData(format!(
            "Palette data block pointer out of bounds: {:#x}, buffer size: {}",
            ptr_palette_data_block, buffer_size
        )));
    }

    // Skip unknown values (Unk#3, nbColorsPerRow, Unk#4, Unk#5)
    read_u16_le(reader).map_err(|e| WanError::Io(e))?; // Unk#3 - ALWAYS 0
    let nb_colors_per_row = read_u16_le(reader).map_err(|e| WanError::Io(e))?;
    read_u16_le(reader).map_err(|e| WanError::Io(e))?; // Unk#4 - ALWAYS 0
    read_u16_le(reader).map_err(|e| WanError::Io(e))?; // Unk#5 - ALWAYS 255

    // Attempt to read palette data with additional error handling
    let palette_data = match read_palette_data(
        reader,
        ptr_palette_data_block as u64,
        ptr_image_data_table as u64,
        16,
    ) {
        Ok(data) => data,
        Err(e) => {
            // If palette reading fails, provide a default palette
            println!("  - Warning: Failed to read palette data: {:?}", e);
            println!("  - Using default palette");
            vec![vec![(0, 0, 0, 0); 16]]
        }
    };

    // Read image data table with bounds checking
    reader
        .seek(SeekFrom::Start(ptr_image_data_table as u64))
        .map_err(|e| WanError::Io(e))?;

    // Read pointers to image data
    let mut ptr_imgs = Vec::with_capacity(nb_imgs as usize);
    for _ in 0..nb_imgs {
        let ptr = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
        ptr_imgs.push(ptr);
    }

    // Read image data with additional error handling
    let img_data = match read_image_data(reader, &ptr_imgs, buffer_size) {
        Ok(data) => data,
        Err(e) => {
            println!("  - Warning: Failed to read image data: {:?}", e);
            println!("  - Using empty image data");
            Vec::new()
        }
    };

    // Special handling for animation info - it might be missing or invalid
    if ptr_anim_info as u64 >= buffer_size - 16 {
        // Need at least 16 bytes for header
        println!("  - Warning: Animation info is missing or invalid");
        // Return a minimal WAN file without animations
        return Ok(WanFile {
            img_data,
            frame_data: Vec::new(),
            animation_groups: Vec::new(),
            offset_data: Vec::new(),
            custom_palette: palette_data,
            sdw_size: 1, // Default shadow size
            wan_type: WanType::Character,
        });
    }

    // Read animation info with bounds checking
    reader
        .seek(SeekFrom::Start(ptr_anim_info as u64))
        .map_err(|e| WanError::Io(e))?;

    let ptr_meta_frames_ref_table = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
    let ptr_offsets_table = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
    let ptr_anim_group_table = read_u32_le(reader).map_err(|e| WanError::Io(e))?;

    // Bounds check these pointers
    if ptr_meta_frames_ref_table as u64 >= buffer_size
        || ptr_offsets_table as u64 >= buffer_size
        || ptr_anim_group_table as u64 >= buffer_size
    {
        return Err(WanError::InvalidData(format!(
            "Animation pointers out of bounds. meta_frames: {:#x}, offsets: {:#x}, anim_group: {:#x}, buffer size: {}",
            ptr_meta_frames_ref_table, ptr_offsets_table, ptr_anim_group_table, buffer_size
        )));
    }

    let nb_anim_groups = read_u16_le(reader).map_err(|e| WanError::Io(e))?;

    // Skip unknown values (Unk#6 through Unk#10)
    for _ in 0..5 {
        read_u16_le(reader).map_err(|e| WanError::Io(e))?;
    }

    // Read animation groups with error handling
    let (animation_groups, anim_sequences) = match read_animation_groups(
        reader,
        ptr_anim_group_table as u64,
        nb_anim_groups as usize,
    ) {
        Ok(result) => result,
        Err(e) => {
            println!("  - Warning: Failed to read animation groups: {:?}", e);
            (Vec::new(), Vec::new())
        }
    };

    // Read meta frames with error handling
    let meta_frames = match read_meta_frames(
        reader,
        ptr_meta_frames_ref_table as u64,
        ptr_offsets_table as u64,
    ) {
        Ok(frames) => frames,
        Err(e) => {
            println!("  - Warning: Failed to read meta frames: {:?}", e);
            Vec::new()
        }
    };

    // Read offset data with error handling
    let offset_data = match read_offset_data(reader, ptr_offsets_table as u64, meta_frames.len()) {
        Ok(offsets) => offsets,
        Err(e) => {
            println!("  - Warning: Failed to read offset data: {:?}", e);
            Vec::new()
        }
    };

    // Read animation sequences with SkyTemple-style approach
    let animation_data =
        match read_animation_sequences(reader, &animation_groups, &anim_sequences)
        {
            Ok(data) => data,
            Err(e) => {
                println!("  - Warning: Failed to read animation sequences: {:?}", e);
                Vec::new()
            }
        };

    let frame_data = meta_frames;

    Ok(WanFile {
        img_data,
        frame_data,
        animation_groups: animation_data,
        offset_data,
        custom_palette: palette_data,
        sdw_size: 1,
        wan_type: WanType::Character,
    })
}

/// Parse an effect WAN file (from effect.bin)
pub fn parse_effect_wan(
    reader: &mut Cursor<&[u8]>,
    buffer_size: u64,
) -> Result<WanFile, WanError> {
    // Read WAN header - same as character WAN
    let ptr_anim_info = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
    let ptr_image_data_info = read_u32_le(reader).map_err(|e| WanError::Io(e))?;

    // Validate pointers
    if ptr_anim_info == 0 || ptr_image_data_info == 0 {
        return Err(WanError::InvalidData(
            "Null pointer in WAN header".to_string(),
        ));
    }

    // Read image type (should be 2 or 3 for effect sprites)
    let img_type = read_u16_le(reader).map_err(|e| WanError::Io(e))?;
    if img_type != 2 && img_type != 3 {
        println!(
            "  - Warning: Effect WAN with unexpected imgType {}",
            img_type
        );
    }

    // Skip unknown value (Unk#12)
    read_u16_le(reader).map_err(|e| WanError::Io(e))?;

    // Read image data info
    reader
        .seek(SeekFrom::Start(ptr_image_data_info as u64))
        .map_err(|e| WanError::Io(e))?;

    let ptr_image_data_table = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
    let ptr_palette_info = read_u32_le(reader).map_err(|e| WanError::Io(e))?;

    // Validate pointers
    if ptr_image_data_table == 0 || ptr_palette_info == 0 {
        return Err(WanError::InvalidData(
            "Null pointer in Image Data Info".to_string(),
        ));
    }

    // Unlike Character WAN, Effect WAN may use 256 colors
    read_u16_le(reader).map_err(|e| WanError::Io(e))?; // Unk#13 - ALWAYS 1
    let is_256_color = read_u16_le(reader).map_err(|e| WanError::Io(e))?;
    read_u16_le(reader).map_err(|e| WanError::Io(e))?; // Unk#11

    // Read number of images
    let nb_imgs = read_u16_le(reader).map_err(|e| WanError::Io(e))?;

    // Read palette info
    reader
        .seek(SeekFrom::Start(ptr_palette_info as u64))
        .map_err(|e| WanError::Io(e))?;

    let ptr_palette_data_block = read_u32_le(reader).map_err(|e| WanError::Io(e))?;

    // Read palette info - different handling for effect WAN
    // Unk#3 - Usually 1 except for effect_0001 - 0
    read_u16_le(reader).map_err(|e| WanError::Io(e))?;

    // Total colors - but may not include all colors in the block
    let total_colors = read_u16_le(reader).map_err(|e| WanError::Io(e))?;

    // Unk#4 - ALWAYS 1 except for effect_0001 - 0
    read_u16_le(reader).map_err(|e| WanError::Io(e))?;

    // Unk#5 - palette offset, ALWAYS 269 except for effect_0001 and effect_0262 - 255
    let palette_offset = read_u16_le(reader).map_err(|e| WanError::Io(e))?;

    // Read palette data for Effect WAN
    let palette_data = match read_effect_palette_data(
        reader,
        ptr_palette_data_block as u64,
        ptr_palette_info as u64,
        is_256_color as usize,
    ) {
        Ok(data) => data,
        Err(e) => {
            println!("  - Warning: Failed to read effect palette data: {:?}", e);
            vec![vec![(0, 0, 0, 0); 16]]
        }
    };

    // Read image data table
    reader
        .seek(SeekFrom::Start(ptr_image_data_table as u64))
        .map_err(|e| WanError::Io(e))?;

    // Read pointers to image data
    let mut ptr_imgs = Vec::with_capacity(nb_imgs as usize);
    for _ in 0..nb_imgs {
        let ptr = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
        ptr_imgs.push(ptr);
    }

    // Determine if we use the specialized imgType 3 handling
    let img_data = if img_type == 3 {
        match read_effect_imgtype3_data(
            reader,
            &ptr_imgs[0..1],
            buffer_size,
            is_256_color as usize,
        ) {
            Ok(data) => data,
            Err(e) => {
                println!("  - Warning: Failed to read effect imgType3 data: {:?}", e);
                Vec::new()
            }
        }
    } else {
        // Standard image data reading
        match read_image_data(reader, &ptr_imgs, buffer_size) {
            Ok(data) => data,
            Err(e) => {
                println!("  - Warning: Failed to read effect image data: {:?}", e);
                Vec::new()
            }
        }
    };

    // Read animation info
    if ptr_anim_info == 0 {
        // Some effect WAN files don't have animation data
        return Ok(WanFile {
            img_data,
            frame_data: Vec::new(),
            animation_groups: Vec::new(),
            offset_data: Vec::new(),
            custom_palette: palette_data,
            sdw_size: 1,
            wan_type: WanType::Effect,
        });
    }

    reader
        .seek(SeekFrom::Start(ptr_anim_info as u64))
        .map_err(|e| WanError::Io(e))?;

    let ptr_meta_frames_ref_table = read_u32_le(reader).map_err(|e| WanError::Io(e))?;

    // Effect WAN doesn't have offsets table
    let ptr_offsets_table = read_u32_le(reader).map_err(|e| WanError::Io(e))?;

    let ptr_anim_group_table = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
    let nb_anim_groups = read_u16_le(reader).map_err(|e| WanError::Io(e))?;

    // Read animation groups
    let (animation_groups, anim_sequences) = match read_animation_groups(
        reader,
        ptr_anim_group_table as u64,
        nb_anim_groups as usize,
    ) {
        Ok(data) => data,
        Err(e) => {
            println!(
                "  - Warning: Failed to read effect animation groups: {:?}",
                e
            );
            (Vec::new(), Vec::new())
        }
    };

    // Read effect meta frames - different format than character WAN
    let meta_frames = match read_effect_meta_frames(
        reader,
        ptr_meta_frames_ref_table as u64,
        ptr_anim_group_table as u64,
    ) {
        Ok(frames) => frames,
        Err(e) => {
            println!("  - Warning: Failed to read effect meta frames: {:?}", e);
            Vec::new()
        }
    };

    // Effect WAN has no offset data
    let offset_data = Vec::new();

    // Read animation sequences using SkyTemple-style approach
    let animation_data =
        match read_animation_sequences(reader, &animation_groups, &anim_sequences)
        {
            Ok(data) => data,
            Err(e) => {
                println!(
                    "  - Warning: Failed to read effect animation sequences: {:?}",
                    e
                );
                Vec::new()
            }
        };

    let frame_data = meta_frames;

    // Combine everything into a WanFile
    Ok(WanFile {
        img_data,
        frame_data,
        animation_groups: animation_data,
        offset_data,
        custom_palette: palette_data,
        sdw_size: 1,
        wan_type: WanType::Effect,
    })
}

/// Read palette data from the WAN file
fn read_palette_data(
    reader: &mut Cursor<&[u8]>,
    ptr_palette_data_block: u64,
    end_ptr: u64, // This is now the end boundary of palette data
    nb_colors_per_row: usize,
) -> Result<Vec<Vec<(u8, u8, u8, u8)>>, WanError> {
    println!(
        "Reading palette data from 0x{:x} to 0x{:x}",
        ptr_palette_data_block, end_ptr
    );

    debug_assert!(
        ptr_palette_data_block > 0,
        "Palette data block pointer is zero"
    );
    debug_assert!(end_ptr > 0, "End pointer is zero");
    debug_assert!(
        end_ptr > ptr_palette_data_block,
        "Invalid palette block range"
    );

    let buffer_size = reader.get_ref().len() as u64;

    // Seek to palette data block
    reader
        .seek(SeekFrom::Start(ptr_palette_data_block))
        .map_err(|e| {
            println!("ERROR: Failed to seek to palette data block");
            WanError::Io(e)
        })?;

    // Calculate total colors based on data block size
    let total_colors = ((end_ptr - ptr_palette_data_block) / 4) as usize;

    // Calculate total palettes
    let total_palettes = if nb_colors_per_row == 0 {
        0
    } else {
        total_colors / nb_colors_per_row
    };
    println!(
        "  Found {} total colors, {} palettes with {} colors per row",
        total_colors, total_palettes, nb_colors_per_row
    );

    let mut custom_palette = Vec::with_capacity(total_palettes);

    for palette_idx in 0..total_palettes {
        let mut palette = Vec::with_capacity(nb_colors_per_row);

        for color_idx in 0..nb_colors_per_row {
            // Read colors in SkyTemple order - red, blue, green
            let red = read_u8(reader).map_err(|e| {
                println!("ERROR: Failed to read red component");
                WanError::Io(e)
            })?;

            let blue = read_u8(reader).map_err(|e| {
                println!("ERROR: Failed to read blue component");
                WanError::Io(e)
            })?;

            let green = read_u8(reader).map_err(|e| {
                println!("ERROR: Failed to read green component");
                WanError::Io(e)
            })?;

            // Skip alpha byte
            let _ = read_u8(reader).map_err(|e| {
                println!("ERROR: Failed to read alpha component");
                WanError::Io(e)
            })?;

            palette.push((red, blue, green, 255));
        }

        // Check if this palette needs completion
        let needs_completion = palette.len() < 16 || palette[0] != (0, 0, 0, 0);

        // Always ensure index 0 is transparent and we have 16 colors
        ensure_complete_palette(&mut palette);

        custom_palette.push(palette);
    }

    // If no palettes were loaded, create a default one
    if custom_palette.is_empty() {
        println!("  No palettes found, creating default palette");
        let mut default_palette = vec![(0, 0, 0, 0)];
        ensure_complete_palette(&mut default_palette);
        custom_palette.push(default_palette);
    }

    return Ok(custom_palette);
}

/// Ensure a palette has all 16 colors, without modifying existing colors
fn ensure_complete_palette(palette: &mut Vec<(u8, u8, u8, u8)>) {
    // If palette is empty, add a single transparent entry
    if palette.is_empty() {
        palette.push((0, 0, 0, 0));
    }

    // If palette has less than 16 colors, pad with better defaults
    // but don't modify any existing colors
    if palette.len() < 16 {
        let default_colors = [
            (128, 128, 128, 255), // Gray
            (192, 192, 192, 255), // Light gray
            (96, 96, 96, 255),    // Dark gray
            (160, 160, 160, 255), // Medium gray
            (224, 224, 224, 255), // Very light gray
            (64, 64, 64, 255),    // Very dark gray
            (128, 96, 64, 255),   // Brown
            (192, 160, 128, 255), // Tan
            (64, 96, 128, 255),   // Blue-gray
            (224, 224, 192, 255), // Cream
            (96, 128, 96, 255),   // Moss green
            (160, 192, 160, 255), // Sage
            (128, 64, 64, 255),   // Brick red
            (192, 128, 128, 255), // Dusty rose
            (255, 255, 255, 255), // White
        ];

        // Add as many colors as needed to reach 16
        let needed = 16 - palette.len();
        for i in 0..needed.min(default_colors.len()) {
            palette.push(default_colors[i]);
        }

        // If we still need more colors, add grayscale
        while palette.len() < 16 {
            let val = ((palette.len() as u8) * 16).min(255);
            palette.push((val, val, val, 255));
        }
    }

    // Trim if we have too many colors
    while palette.len() > 16 {
        palette.pop();
    }
}

/// Read palette data for Effect WAN with special handling for 256-color mode
fn read_effect_palette_data(
    reader: &mut Cursor<&[u8]>,
    ptr_palette_data_block: u64,
    ptr_palette_info: u64,
    is_256_color: usize,
) -> Result<Vec<Vec<(u8, u8, u8, u8)>>, WanError> {
    // Seek directly to palette data block
    reader
        .seek(SeekFrom::Start(ptr_palette_data_block))
        .map_err(|e| WanError::Io(e))?;

    let total_bytes = ptr_palette_info - ptr_palette_data_block;
    let mut custom_palette = Vec::new();

    if is_256_color == 4 {
        // Special case seen in effect267
        let nb_colors_per_row = 256;
        let mut palette = vec![(0, 0, 0, 0); nb_colors_per_row];

        let total_colors = total_bytes / 4;
        for jj in 0..total_colors as usize {
            let red = read_u8(reader).map_err(|e| WanError::Io(e))? / 8 * 8 * 32 / 31;
            let blue = read_u8(reader).map_err(|e| WanError::Io(e))? / 8 * 8 * 32 / 31;
            let green = read_u8(reader).map_err(|e| WanError::Io(e))? / 8 * 8 * 32 / 31;
            read_u8(reader).map_err(|e| WanError::Io(e))?; // Skip alpha

            if 16 + jj < nb_colors_per_row {
                palette[16 + jj] = (red, blue, green, 255);
            }
        }
        custom_palette.push(palette);
    } else if is_256_color == 1 {
        // 8bpp = 2^8 colors
        let nb_colors_per_row = 256;
        let nb_reads_per_row = 16;
        let total_colors = (total_bytes / 4) as usize;
        let total_palettes = total_colors / nb_reads_per_row;

        for _ in 0..total_palettes {
            let mut palette = vec![(0, 0, 0, 0); nb_colors_per_row];
            for jj in 0..nb_reads_per_row {
                let red = read_u8(reader).map_err(|e| WanError::Io(e))? / 8 * 8 * 32 / 31;
                let blue = read_u8(reader).map_err(|e| WanError::Io(e))? / 8 * 8 * 32 / 31;
                let green = read_u8(reader).map_err(|e| WanError::Io(e))? / 8 * 8 * 32 / 31;
                read_u8(reader).map_err(|e| WanError::Io(e))?; // Skip alpha

                palette[16 + jj] = (red, blue, green, 255);
            }
            custom_palette.push(palette);
        }
    } else {
        // 4bpp = 2^4 colors
        let nb_colors_per_row = 16;
        let total_colors = (total_bytes / 4) as usize;
        let total_palettes = total_colors / nb_colors_per_row;

        for _ in 0..total_palettes {
            let mut palette = Vec::with_capacity(nb_colors_per_row);
            for _ in 0..nb_colors_per_row {
                let red = read_u8(reader).map_err(|e| WanError::Io(e))?;
                let blue = read_u8(reader).map_err(|e| WanError::Io(e))?;
                let green = read_u8(reader).map_err(|e| WanError::Io(e))?;
                read_u8(reader).map_err(|e| WanError::Io(e))?; // Skip alpha

                palette.push((red, blue, green, 255));
            }
            custom_palette.push(palette);
        }
    }

    // Return at least one palette even if empty
    if custom_palette.is_empty() {
        custom_palette.push(vec![(0, 0, 0, 255); 16]);
    }

    Ok(custom_palette)
}

/// Read image data from the WAN file
fn read_image_data(
    reader: &mut Cursor<&[u8]>,
    ptr_imgs: &[u32],
    buffer_size: u64,
) -> Result<Vec<ImgPiece>, WanError> {
    let mut img_data = Vec::with_capacity(ptr_imgs.len());

    println!(
        "Reading image data: {} pointers, buffer size: {} bytes",
        ptr_imgs.len(),
        buffer_size
    );

    for (img_idx, &ptr_img) in ptr_imgs.iter().enumerate() {
        // Use pointer directly - now correctly pointing to file position
        if let Err(e) = reader.seek(SeekFrom::Start(ptr_img as u64)) {
            println!(
                "  - Warning: Failed to seek to image data for image #{}: {}",
                img_idx, e
            );
            img_data.push(ImgPiece {
                img_px: Vec::new(),
                z_sort: 0,
            });
            continue;
        }

        let mut img_piece = ImgPiece {
            img_px: Vec::new(),
            z_sort: 0,
        };
        let mut valid_data = false;
        let mut total_pixels_read = 0;
        let mut total_pixels_expected = 0;
        let mut zero_filled_sections = 0;
        let mut data_filled_sections = 0;

        // Read image data sections
        loop {
            // Read header values - handle errors gracefully
            let ptr_pix_src = match read_u32_le(reader) {
                Ok(val) => val,
                Err(e) => {
                    if img_piece.img_px.is_empty() {
                        println!(
                            "  - Warning: Failed to read pixel source pointer for image #{}: {}",
                            img_idx, e
                        );
                    }
                    break;
                }
            };

            let amt = match read_u16_le(reader) {
                Ok(val) => val,
                Err(e) => {
                    println!(
                        "  - Warning: Failed to read pixel amount for image #{}: {}",
                        img_idx, e
                    );
                    break;
                }
            };

            // End of sections marker
            if ptr_pix_src == 0 && amt == 0 {
                break;
            }

            // Skip Unk#14
            if let Err(e) = read_u16_le(reader) {
                println!(
                    "  - Warning: Failed to read unknown field for image #{}: {}",
                    img_idx, e
                );
                break;
            }

            // Read z-sort value
            img_piece.z_sort = match read_u32_le(reader) {
                Ok(val) => val,
                Err(e) => {
                    println!(
                        "  - Warning: Failed to read z-sort value for image #{}: {}",
                        img_idx, e
                    );
                    0
                }
            };

            // Track expected pixels for diagnostics
            total_pixels_expected += amt as usize;

            // Handle pixels
            let mut px_strip = Vec::with_capacity(amt as usize);
            let mut pixels_read_in_strip = 0;

            // KEY CHANGE: Only zero-fill when pixel source pointer is zero
            // This matches SkyTemple's character wan implementation
            if ptr_pix_src == 0 {
                // Zero padding case - only when pixel source is zero
                for _ in 0..amt {
                    px_strip.push(0);
                    pixels_read_in_strip += 1;
                }
                valid_data = true;
                zero_filled_sections += 1;
                
                println!("  - Image #{}: Using zero-fill for {} pixels (ptr=0)", img_idx, amt);
            } else {
                // Save current position to return to after reading pixels
                let current_pos = reader.position();

                // Use pixel source pointer directly
                if let Err(e) = reader.seek(SeekFrom::Start(ptr_pix_src as u64)) {
                    println!(
                        "  - Warning: Failed to seek to pixel data at {:#x} for image #{}: {}",
                        ptr_pix_src, img_idx, e
                    );
                    if let Err(seek_e) = reader.seek(SeekFrom::Start(current_pos)) {
                        println!("  - Warning: Failed to restore position: {}", seek_e);
                    }
                    continue;
                }

                data_filled_sections += 1;
                
                // Read actual pixels with improved diagnostics - KEEP partial reads
                for px_idx in 0..amt {
                    match read_u8(reader) {
                        Ok(px) => {
                            // Store raw byte (don't unpack now!)
                            px_strip.push(px);
                            pixels_read_in_strip += 1;
                            valid_data = true;
                        }
                        Err(e) => {
                            println!(
                                "  - Warning: Partial read for image #{} at position {}: {} (collected {} of {} pixels)",
                                img_idx, 
                                reader.position(), 
                                e,
                                pixels_read_in_strip,
                                amt
                            );
                            // Just break but KEEP what we've read so far
                            break;
                        }
                    }
                }

                total_pixels_read += pixels_read_in_strip;

                // Return to section position
                if let Err(e) = reader.seek(SeekFrom::Start(current_pos)) {
                    println!("  - Warning: Failed to restore position after reading pixels for image #{}: {}", 
                             img_idx, e);
                    break;
                }
            }

            // Add the strip as long as we have ANY data (even partial)
            if !px_strip.is_empty() {
                img_piece.img_px.push(px_strip);
            }
        }

        // Only add if we read some valid data
        if valid_data && !img_piece.img_px.is_empty() {
            println!(
                "  - Image #{}: Read {} pixel strips, {} out of {} pixels ({:.1}%), zero-filled sections: {}, data sections: {}",
                img_idx,
                img_piece.img_px.len(),
                total_pixels_read,
                total_pixels_expected,
                if total_pixels_expected > 0 {
                    (total_pixels_read as f64 / total_pixels_expected as f64) * 100.0
                } else {
                    0.0
                },
                zero_filled_sections,
                data_filled_sections
            );
            img_data.push(img_piece);
        } else {
            println!(
                "  - No valid pixel data for image #{}, adding empty placeholder",
                img_idx
            );
            img_data.push(ImgPiece {
                img_px: Vec::new(),
                z_sort: 0,
            });
        }
    }

    println!("  - Successfully loaded {} image pieces", img_data.len());
    Ok(img_data)
}

/// Read image data for Effect WAN with imgType 3
fn read_effect_imgtype3_data(
    reader: &mut Cursor<&[u8]>,
    ptr_imgs: &[u32],
    buffer_size: u64,
    is_256_color: usize,
) -> Result<Vec<ImgPiece>, WanError> {
    let mut img_data = Vec::new();

    // Use pointer directly - now correctly pointing to file position
    reader
        .seek(SeekFrom::Start(ptr_imgs[0] as u64))
        .map_err(|e| WanError::Io(e))?;

    let mut img_piece = ImgPiece {
        img_px: Vec::new(),
        z_sort: 0,
    };

    let ptr_pix_src = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
    let atlas_width = read_u16_le(reader).map_err(|e| WanError::Io(e))?;
    let atlas_height = read_u16_le(reader).map_err(|e| WanError::Io(e))?;
    img_piece.z_sort = read_u32_le(reader).map_err(|e| WanError::Io(e))?;

    // Use pixel source pointer directly - now correctly pointing to file position
    reader
        .seek(SeekFrom::Start(ptr_pix_src as u64))
        .map_err(|e| WanError::Io(e))?;

    let mut px_strip = Vec::new();
    while reader.position() < ptr_imgs[0] as u64 {
        let px = read_u8(reader).map_err(|e| WanError::Io(e))?;

        // Handle different color modes
        if is_256_color == 0 {
            // 4bpp mode - split each byte into two 4-bit values
            px_strip.push(px % 16); // Lower 4 bits
            px_strip.push(px / 16); // Upper 4 bits
        } else {
            // 8bpp mode - one byte per pixel
            px_strip.push(px);
        }
    }

    img_piece.img_px.push(px_strip);
    img_data.push(img_piece);

    Ok(img_data)
}

/// Read meta frames from the WAN file
fn read_meta_frames(
    reader: &mut Cursor<&[u8]>,
    ptr_meta_frames_ref_table: u64,
    ptr_frames_ref_table_end: u64,
) -> Result<Vec<MetaFrame>, WanError> {
    reader
        .seek(SeekFrom::Start(ptr_meta_frames_ref_table))
        .map_err(|e| WanError::Io(e))?;

    // Read pointers to meta frames
    let mut ptr_meta_frames = Vec::new();
    while reader.position() < ptr_frames_ref_table_end {
        let ptr = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
        ptr_meta_frames.push(ptr);
    }

    println!("Decoded {} meta frame pointers", ptr_meta_frames.len());

    // Read meta frames
    let mut meta_frames = Vec::with_capacity(ptr_meta_frames.len());
    let buffer_size = reader.get_ref().len() as u64;

    for (frame_idx, &ptr_meta_frame) in ptr_meta_frames.iter().enumerate() {
        // Log frame start and seek position
        println!(
            "Reading meta frame #{} - seeking to position 0x{:x}",
            frame_idx, ptr_meta_frame
        );

        // Use meta frame pointer directly
        match reader.seek(SeekFrom::Start(ptr_meta_frame as u64)) {
            Ok(_) => {},
            Err(e) => {
                println!("  ERROR: Failed to seek to frame position 0x{:x}: {}", ptr_meta_frame, e);
                return Err(WanError::Io(e));
            }
        }

        let mut meta_frame_pieces = Vec::new();
        let mut minus_frame_refs = 0;

        loop {
            let current_piece_start_pos = reader.position(); // Position BEFORE reading img_index
            
            // Read img_index with error handling
            let img_index = match read_i16_le(reader) {
                Ok(val) => { 
                    if val < 0 { minus_frame_refs += 1; }
                    val 
                },
                Err(e) => { 
                    println!("    ERROR Reading img_index at position 0x{:x}: {}", 
                             current_piece_start_pos, e); 
                    return Err(WanError::Io(e));
                }
            };
            
            println!(
                "  Frame {}, Piece Start @ 0x{:x}, Read img_index: {} (0x{:04x})",
                frame_idx, current_piece_start_pos, img_index, img_index as u16
            );

            // Read other attributes with error handling
            let unk0 = match read_u16_le(reader) {
                Ok(v) => v,
                Err(e) => {
                    println!("    ERROR Reading unk0 at position 0x{:x}: {}", 
                             reader.position(), e);
                    return Err(WanError::Io(e));
                }
            };
            
            let attr0 = match read_u16_le(reader) {
                Ok(v) => v,
                Err(e) => {
                    println!("    ERROR Reading attr0 at position 0x{:x}: {}", 
                             reader.position(), e);
                    return Err(WanError::Io(e));
                }
            };
            
            let attr1 = match read_u16_le(reader) {
                Ok(v) => v,
                Err(e) => {
                    println!("    ERROR Reading attr1 at position 0x{:x}: {}", 
                             reader.position(), e);
                    return Err(WanError::Io(e));
                }
            };
            
            let attr2 = match read_u16_le(reader) {
                Ok(v) => v,
                Err(e) => {
                    println!("    ERROR Reading attr2 at position 0x{:x}: {}", 
                             reader.position(), e);
                    return Err(WanError::Io(e));
                }
            };

            let current_piece_end_pos = reader.position(); // Position AFTER reading all piece data
            let bytes_read_this_piece = current_piece_end_pos - current_piece_start_pos;
            
            println!(
                "    Unk0: 0x{:04x}, Attr0: 0x{:04x}, Attr1: 0x{:04x}, Attr2: 0x{:04x}, End @ 0x{:x} (Read {} bytes)",
                unk0, attr0, attr1, attr2, current_piece_end_pos, bytes_read_this_piece
            );

            let is_last = (attr1 & super::flags::ATTR1_IS_LAST_MASK) != 0;
            
            // Create the piece with the signed img_index value
            meta_frame_pieces.push(MetaFramePiece::new(img_index, attr0, attr1, attr2));

            // Check piece size consistency
            if bytes_read_this_piece != 10 {
                println!(
                    "    !!!! WARNING: Expected 10 bytes read for piece, got {} !!!!",
                    bytes_read_this_piece
                );
            }

            // Check if this is the last piece
            if is_last {
                println!("    Found last piece marker in Attr1: 0x{:04x}", attr1);
                break;
            }
        }

        // Frame summary
        println!(
            "Frame #{} completed with {} pieces, {} MINUS_FRAME references", 
            frame_idx, 
            meta_frame_pieces.len(),
            minus_frame_refs
        );

        meta_frames.push(MetaFrame {
            pieces: meta_frame_pieces,
        });
    }

    // Count MINUS_FRAME references for debugging
    let mut minus_frame_count = 0;
    for frame in &meta_frames {
        for piece in &frame.pieces {
            if piece.img_index < 0 {
                minus_frame_count += 1;
            }
        }
    }
    println!("Total MINUS_FRAME references found: {}", minus_frame_count);

    Ok(meta_frames)
}

/// Read effect meta frames - completely different format from character WAN
fn read_effect_meta_frames(
    reader: &mut Cursor<&[u8]>,
    ptr_meta_frames_ref_table: u64,
    ptr_anim_seq_table: u64,
) -> Result<Vec<MetaFrame>, WanError> {
    reader
        .seek(SeekFrom::Start(ptr_meta_frames_ref_table))
        .map_err(|e| WanError::Io(e))?;

    // Read pointers to meta frames
    let mut ptr_meta_frames = Vec::new();
    while reader.position() < ptr_anim_seq_table {
        let ptr = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
        ptr_meta_frames.push(ptr);
    }

    // Read meta frames
    let mut meta_frames = Vec::with_capacity(ptr_meta_frames.len());
    let buffer_size = reader.get_ref().len() as u64;

    for (frame_idx, &ptr_meta_frame) in ptr_meta_frames.iter().enumerate() {
        // Use meta frame pointer directly - now correctly pointing to file position
        reader
            .seek(SeekFrom::Start(ptr_meta_frame as u64))
            .map_err(|e| WanError::Io(e))?;

        let mut meta_frame_pieces = Vec::new();

        // Effect WAN metaframe format is completely different
        loop {
            // Read first 2 bytes - should be FFFF
            let magic = read_u16_le(reader).map_err(|e| WanError::Io(e))?;
            if magic != 0xFFFF {
                // Not a valid metaframe, or we've reached the end
                break;
            }

            // Read section 1 - should be 0000
            let section1 = read_u16_le(reader).map_err(|e| WanError::Io(e))?;

            // Read section 2 - 00 or FB (draw behind character)
            let draw_behind = read_u8(reader).map_err(|e| WanError::Io(e))? == 0xFB;

            // Read section 3 - Y offset
            let y_offset_lower = read_u8(reader).map_err(|e| WanError::Io(e))?;

            // Read section 4 - Flags including upper bits of Y offset
            let section4 = read_u8(reader).map_err(|e| WanError::Io(e))?;
            // Cast to u16 before shifting to avoid overflow
            let y_offset_upper = ((section4 & 0x03) as u16) << 8;
            let y_offset = y_offset_lower as u16 | y_offset_upper;

            // Read section 5 - X offset
            let x_offset_lower = read_u8(reader).map_err(|e| WanError::Io(e))?;

            // Read section 6 - Flags including size, flips, and upper bits of X offset
            let section6 = read_u8(reader).map_err(|e| WanError::Io(e))?;
            let size_bits = section6 & 0x03;
            let flip_vertical = (section6 & 0x04) != 0;
            let flip_horizontal = (section6 & 0x08) != 0;
            let is_last = (section6 & 0x10) != 0;
            // Cast to u16 before shifting to avoid overflow
            let x_offset_upper = ((section6 & 0x80) as u16) << 1;
            let x_offset = x_offset_lower as u16 | x_offset_upper;

            // Read section 7 - Image offset
            let image_offset = read_u8(reader).map_err(|e| WanError::Io(e))?;

            // Read section 8 - Palette index
            let palette_index = read_u8(reader).map_err(|e| WanError::Io(e))?;

            // Read section 9 - Should be 0x0C
            let section9 = read_u8(reader).map_err(|e| WanError::Io(e))?;

            // Convert the effect metaframe to a format compatible with our MetaFramePiece struct
            // We need to create attr0, attr1, attr2 values that represent the same information

            // Set attributes based on effect metaframe data
            let attr0 = y_offset & 0x03FF; // Y offset in lower 10 bits

            let mut attr1 = x_offset & 0x01FF; // X offset in lower 9 bits
            if flip_horizontal {
                attr1 |= super::flags::ATTR1_HFLIP_MASK;
            }
            if flip_vertical {
                attr1 |= super::flags::ATTR1_VFLIP_MASK;
            }
            if is_last {
                attr1 |= super::flags::ATTR1_IS_LAST_MASK;
            }

            // Convert size to resolution type (0-11)
            // Size bits: 00=8x8, 01=16x16, 10=32x32, 11=64x64
            let res_type = match size_bits {
                0 => 0, // 8x8
                1 => 1, // 16x16
                2 => 2, // 32x32
                3 => 3, // 64x64
                _ => 0,
            };

            // Set resolution in attr0 and attr1
            attr1 |= ((res_type & 0x03) << 14) as u16;

            let attr2 = ((palette_index as u16) << 12) | (image_offset as u16);

            meta_frame_pieces.push(MetaFramePiece::new(
                image_offset as i16,
                attr0,
                attr1,
                attr2,
            ));

            if is_last {
                break;
            }
        }

        if !meta_frame_pieces.is_empty() {
            meta_frames.push(MetaFrame {
                pieces: meta_frame_pieces,
            });
        }
    }

    Ok(meta_frames)
}

/// Read offset data from the WAN file
fn read_offset_data(
    reader: &mut Cursor<&[u8]>,
    ptr_offsets_table: u64,
    num_frames: usize,
) -> Result<Vec<FrameOffset>, WanError> {
    reader
        .seek(SeekFrom::Start(ptr_offsets_table))
        .map_err(|e| WanError::Io(e))?;

    let mut offset_data = Vec::with_capacity(num_frames);

    for _ in 0..num_frames {
        let head_x = read_i16_le(reader).map_err(|e| WanError::Io(e))?;
        let head_y = read_i16_le(reader).map_err(|e| WanError::Io(e))?;

        let lhand_x = read_i16_le(reader).map_err(|e| WanError::Io(e))?;
        let lhand_y = read_i16_le(reader).map_err(|e| WanError::Io(e))?;

        let rhand_x = read_i16_le(reader).map_err(|e| WanError::Io(e))?;
        let rhand_y = read_i16_le(reader).map_err(|e| WanError::Io(e))?;

        let center_x = read_i16_le(reader).map_err(|e| WanError::Io(e))?;
        let center_y = read_i16_le(reader).map_err(|e| WanError::Io(e))?;

        offset_data.push(FrameOffset::new(
            (head_x, head_y),
            (lhand_x, lhand_y),
            (rhand_x, rhand_y),
            (center_x, center_y),
        ));
    }

    Ok(offset_data)
}

/// Read animation groups from the WAN file, matching SkyTemple's approach
fn read_animation_groups(
    reader: &mut Cursor<&[u8]>,
    ptr_anim_group_table: u64,
    num_anim_groups: usize,
) -> Result<(Vec<Vec<u32>>, Vec<u32>), WanError> {
    reader
        .seek(SeekFrom::Start(ptr_anim_group_table))
        .map_err(|e| WanError::Io(e))?;

    let mut anim_groups: Vec<Vec<u32>> = Vec::with_capacity(num_anim_groups);
    let mut anim_sequences: Vec<u32> = Vec::new();
    let buffer_size = reader.get_ref().len() as u64;

    println!(
        "Reading {} animation groups from {:#x}",
        num_anim_groups, ptr_anim_group_table
    );

    for group_idx in 0..num_anim_groups {
        let anim_loc = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
        let anim_length = read_u16_le(reader).map_err(|e| WanError::Io(e))?;

        // Skip Unk#16
        read_u16_le(reader).map_err(|e| WanError::Io(e))?;

        // Save current position
        let current_pos = reader.position();

        // Skip empty groups
        if anim_loc == 0 || anim_length == 0 || anim_loc as u64 >= buffer_size {
            println!("  - Group {}: Empty or invalid", group_idx);
            anim_groups.push(Vec::new());
            continue;
        }

        println!(
            "  - Group {}: {} animations at {:#x}",
            group_idx, anim_length, anim_loc
        );

        // Use animation location pointer directly - now correctly pointing to file position
        reader
            .seek(SeekFrom::Start(anim_loc as u64))
            .map_err(|e| WanError::Io(e))?;

        let mut anim_ptrs = Vec::with_capacity(anim_length as usize);

        // Read all animation pointers in this group
        for dir_idx in 0..anim_length {
            let anim_ptr = read_u32_le(reader).map_err(|e| WanError::Io(e))?;
            anim_ptrs.push(anim_ptr);
            anim_sequences.push(anim_ptr);

            println!("    - Direction {}: Animation at {:#x}", dir_idx, anim_ptr);
        }

        anim_groups.push(anim_ptrs);

        // Restore position
        reader
            .seek(SeekFrom::Start(current_pos))
            .map_err(|e| WanError::Io(e))?;
    }

    println!(
        "Read {} animation groups with {} total animations",
        anim_groups.len(),
        anim_sequences.len()
    );

    Ok((anim_groups, anim_sequences))
}

/// Read animation sequences from the WAN file, following SkyTemple's direct approach
fn read_animation_sequences(
    reader: &mut Cursor<&[u8]>,
    animation_groups: &[Vec<u32>],
    _anim_sequences: &[u32], // We keep this parameter for API compatibility
) -> Result<Vec<Vec<Animation>>, WanError> {
    let buffer_size = reader.get_ref().len() as u64;

    println!(
        "Reading animation sequences from {} groups",
        animation_groups.len()
    );

    // Create the result array to match the animation group structure
    let mut result_animation_groups: Vec<Vec<Animation>> = 
        Vec::with_capacity(animation_groups.len());

    // Process each animation group
    for (group_idx, group) in animation_groups.iter().enumerate() {
        println!(
            "  Processing animation group {}: {} animations",
            group_idx,
            group.len()
        );

        // Skip empty groups but preserve structure with empty Vec
        if group.is_empty() {
            result_animation_groups.push(Vec::new());
            continue;
        }

        // Create a new animation group
        let mut result_group: Vec<Animation> = Vec::with_capacity(group.len());

        // Process each animation pointer in this group
        for (dir_idx, &ptr) in group.iter().enumerate() {
            // SkyTemple's optimization: skip duplicate pointers within the same group
            if dir_idx > 0 && ptr == group[dir_idx - 1] {
                // Clone the previous animation
                if !result_group.is_empty() {
                    result_group.push(result_group[dir_idx - 1].clone());
                    println!("    Direction {}: Reusing previous animation", dir_idx);
                    continue;
                }
            }

            println!(
                "    Reading animation at direction {} from address {:#x}",
                dir_idx, ptr
            );

            // Use animation sequence pointer directly
            if let Err(e) = reader.seek(SeekFrom::Start(ptr as u64)) {
                println!(
                    "    Error seeking to animation sequence at {:#x}: {}",
                    ptr, e
                );
                // Add an empty animation as a placeholder
                result_group.push(Animation::empty());
                continue;
            }

            // Read sequence frames
            let mut sequence_frames = Vec::new();
            let mut frame_count = 0;

            // Read the animation sequence - mirroring SkyTemple's while True: loop
            loop {
                // Read frame duration - 0 marks end of sequence
                let frame_dur = match read_u8(reader) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("    Error reading frame duration: {}", e);
                        break;
                    }
                };

                // End of sequence marker
                if frame_dur == 0 {
                    // Skip remaining bytes of the end marker
                    let mut skip_buf = [0u8; 11];
                    if let Err(e) = reader.read_exact(&mut skip_buf) {
                        println!("    Warning: Error skipping end marker: {}", e);
                        // Non-fatal error, we've already detected the end
                    }
                    break;
                }

                // Read rest of frame data
                let flag = match read_u8(reader) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("    Error reading frame flag: {}", e);
                        break;
                    }
                };

                let frame_index = match read_u16_le(reader) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("    Error reading frame index: {}", e);
                        break;
                    }
                };

                let spr_off_x = match read_i16_le(reader) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("    Error reading sprite offset X: {}", e);
                        break;
                    }
                };

                let spr_off_y = match read_i16_le(reader) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("    Error reading sprite offset Y: {}", e);
                        break;
                    }
                };

                let sdw_off_x = match read_i16_le(reader) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("    Error reading shadow offset X: {}", e);
                        break;
                    }
                };

                let sdw_off_y = match read_i16_le(reader) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("    Error reading shadow offset Y: {}", e);
                        break;
                    }
                };

                // Create frame with proper validation
                sequence_frames.push(SequenceFrame::new(
                    frame_index,
                    frame_dur,
                    flag,
                    (spr_off_x, spr_off_y),
                    (sdw_off_x, sdw_off_y),
                ));

                frame_count += 1;
            }

            println!("    Direction {}: Read {} frames", dir_idx, frame_count);

            // Add the animation to the group
            result_group.push(Animation::new(sequence_frames));
        }

        // Add the group to the result
        result_animation_groups.push(result_group.clone());
        println!(
            "  Group {} completed with {} animations",
            group_idx,
            result_group.len()
        );
    }

    // Log detailed information about the animation groups
    println!(
        "Created {} animation groups:",
        result_animation_groups.len()
    );
    for (i, group) in result_animation_groups.iter().enumerate() {
        println!("  Group {}: {} animations", i, group.len());

        // Log the number of frames in each animation (direction)
        for (j, anim) in group.iter().enumerate() {
            println!("    Direction {}: {} frames", j, anim.frames.len());
        }
    }

    Ok(result_animation_groups)
}

pub fn map_img_data(input_img_data: &[ImgPiece], frame_data: &mut [MetaFrame]) -> Vec<ImgPiece> {
    let mut img_data = Vec::new();
    let mut mapping = HashMap::new();

    // First pass: identify unique image pieces
    for (idx, img) in input_img_data.iter().enumerate() {
        let mut dupe_idx = None;

        // Check if this image is a duplicate of an existing one
        for (check_idx, check_img) in img_data.iter().enumerate() {
            if is_img_piece_equal(img, check_img) {
                dupe_idx = Some(check_idx);
                break;
            }
        }

        // Map this index to either the duplicate or a new entry
        if let Some(existing_idx) = dupe_idx {
            mapping.insert(idx, existing_idx);
        } else {
            mapping.insert(idx, img_data.len());
            img_data.push(img.clone());
        }
    }

    // Second pass: update frame references
    for frame in frame_data.iter_mut() {
        for piece in &mut frame.pieces {
            if piece.img_index >= 0 {
                if let Some(&new_idx) = mapping.get(&(piece.img_index as usize)) {
                    piece.img_index = new_idx as i16;
                }
            }
        }
    }

    img_data
}

/// Helper to check if two image pieces are identical
fn is_img_piece_equal(img1: &ImgPiece, img2: &ImgPiece) -> bool {
    if img1.z_sort != img2.z_sort || img1.img_px.len() != img2.img_px.len() {
        return false;
    }

    for (strip1, strip2) in img1.img_px.iter().zip(img2.img_px.iter()) {
        if strip1.len() != strip2.len() {
            return false;
        }

        for (px1, px2) in strip1.iter().zip(strip2.iter()) {
            if px1 != px2 {
                return false;
            }
        }
    }

    true
}
