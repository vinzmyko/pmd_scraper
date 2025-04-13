//! Parser for WAN sprite format
//!
//! This module provides functions to parse WAN files from binary data,
//! supporting both character and effect WAN variants.

use std::io::{self, Cursor, Read, Seek, SeekFrom};

use crate::graphics::{
    wan::{
        model::{
            Animation, FrameOffset, ImgPiece, MetaFrame, MetaFramePiece, SequenceFrame, WanFile
        },
        WanError
    }, WanType,
};

pub fn read_u8(cursor: &mut Cursor<&[u8]>) -> io::Result<u8> {
    if cursor.position() >= cursor.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "End of buffer reached",
        ));
    }

    let mut buf = [0u8; 1];
    cursor.read_exact(&mut buf)?;
    Ok(buf[0])
}

pub fn read_u16_le(cursor: &mut Cursor<&[u8]>) -> io::Result<u16> {
    if cursor.position() + 1 >= cursor.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "End of buffer reached or not enough bytes for u16",
        ));
    }

    let mut buf = [0u8; 2];
    cursor.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

pub fn read_u32_le(cursor: &mut Cursor<&[u8]>) -> io::Result<u32> {
    if cursor.position() + 3 >= cursor.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "End of buffer reached or not enough bytes for u32",
        ));
    }

    let mut buf = [0u8; 4];
    cursor.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

/// Read an i16 in little-endian format from the cursor
pub fn read_i16_le(cursor: &mut Cursor<&[u8]>) -> Result<i16, io::Error> {
    if cursor.position() + 1 >= cursor.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "End of buffer reached or not enough bytes for i16",
        ));
    }

    let mut buf = [0u8; 2];
    cursor.read_exact(&mut buf)?;
    Ok(i16::from_le_bytes(buf))
}

/// Parse WAN file from SIR0 content that has already been extracted
pub fn parse_wan_from_sir0_content(
    content: &[u8],
    data_pointer: u32,
    wan_type: WanType,
) -> Result<WanFile, WanError> {
    let mut cursor = Cursor::new(content);
    let buffer_size = content.len() as u64;

    // Read from the data pointer position
    cursor
        .seek(SeekFrom::Start(data_pointer as u64))
        .map_err(|e| WanError::Io(e))?;

    match wan_type {
        WanType::Character => parse_character_wan(&mut cursor, buffer_size),
        WanType::Effect => parse_effect_wan(&mut cursor, buffer_size),
    }
}

pub fn parse_character_wan(
    cursor: &mut Cursor<&[u8]>,
    buffer_size: u64,
) -> Result<WanFile, WanError> {
    // Store current position to check for minimal header
    let start_pos = cursor.position();

    // Make sure we have enough bytes for the basic header
    if start_pos + 8 > buffer_size {
        return Err(WanError::InvalidDataStructure(format!(
            "Not enough bytes for WAN header. Position: {}, Buffer size: {}",
            start_pos, buffer_size
        )));
    }

    let ptr_anim_info = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;
    let ptr_image_data_info = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;

    // Validate pointers
    if ptr_anim_info == 0 || ptr_image_data_info == 0 {
        return Err(WanError::InvalidDataStructure(
            "Null pointer in WAN header".to_string(),
        ));
    }

    if ptr_anim_info as u64 >= buffer_size || ptr_image_data_info as u64 >= buffer_size {
        return Err(WanError::InvalidDataStructure(format!(
            "Pointer out of bounds. anim_info: {:#x}, image_data_info: {:#x}, buffer size: {}",
            ptr_anim_info, ptr_image_data_info, buffer_size
        )));
    }

    // Should be 1 for character sprites
    let img_type = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;
    if img_type != 1 {
        return Err(WanError::InvalidDataStructure(format!(
            "Expected image type 1 for character sprite, got {}",
            img_type
        )));
    }

    // Skip unknown value
    read_u16_le(cursor).map_err(|e| WanError::Io(e))?;

    // Read image data info with bounds checking
    if ptr_image_data_info as u64 >= buffer_size {
        return Err(WanError::InvalidDataStructure(format!(
            "Image data info pointer out of bounds: {:#x}, buffer size: {}",
            ptr_image_data_info, buffer_size
        )));
    }

    cursor
        .seek(SeekFrom::Start(ptr_image_data_info as u64))
        .map_err(|e| WanError::Io(e))?;

    let ptr_image_data_table = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;
    let ptr_palette_info = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;

    // Validate pointers
    if ptr_image_data_table == 0 || ptr_palette_info == 0 {
        return Err(WanError::InvalidDataStructure(
            "Null pointer in Image Data Info".to_string(),
        ));
    }

    // Bounds check
    if ptr_image_data_table as u64 >= buffer_size || ptr_palette_info as u64 >= buffer_size {
        return Err(WanError::InvalidDataStructure(format!(
            "Pointer out of bounds. image_data_table: {:#x}, palette_info: {:#x}, buffer size: {}",
            ptr_image_data_table, ptr_palette_info, buffer_size
        )));
    }

    // Skip unknown values Unk#13, Is256ColorSpr, Unk#11
    read_u16_le(cursor).map_err(|e| WanError::Io(e))?; // Unk#13 - ALWAYS 0
    read_u16_le(cursor).map_err(|e| WanError::Io(e))?; // Is256ColorSpr - ALWAYS 0
    read_u16_le(cursor).map_err(|e| WanError::Io(e))?; // Unk#11 - ALWAYS 1 unless empty

    // Read number of images
    let num_imgs = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;

    // Read palette info with bounds checking
    cursor
        .seek(SeekFrom::Start(ptr_palette_info as u64))
        .map_err(|e| WanError::Io(e))?;

    let ptr_palette_data_block = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;

    // Bounds check palette data block
    if ptr_palette_data_block as u64 >= buffer_size {
        return Err(WanError::InvalidDataStructure(format!(
            "Palette data block pointer out of bounds: {:#x}, buffer size: {}",
            ptr_palette_data_block, buffer_size
        )));
    }

    // Skip unknown values (Unk#3, nbColorsPerRow, Unk#4, Unk#5)
    read_u16_le(cursor).map_err(|e| WanError::Io(e))?; // Unk#3 - ALWAYS 0
    let _colours_per_row_num = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;
    read_u16_le(cursor).map_err(|e| WanError::Io(e))?; // Unk#4 - ALWAYS 0
    read_u16_le(cursor).map_err(|e| WanError::Io(e))?; // Unk#5 - ALWAYS 255

    let palette_data = match read_palette_data(
        cursor,
        ptr_palette_data_block as u64,
        ptr_image_data_table as u64,
        16,
    ) {
        Ok(data) => data,
        Err(e) => {
            println!("  - Warning: Failed to read palette data: {:?}", e);
            println!("  - Using default palette");
            vec![vec![(0, 0, 0, 0); 16]]
        }
    };

    // Read image data table
    cursor
        .seek(SeekFrom::Start(ptr_image_data_table as u64))
        .map_err(|e| WanError::Io(e))?;

    // Read pointers to image data
    let mut ptr_imgs = Vec::with_capacity(num_imgs as usize);
    for _ in 0..num_imgs {
        let ptr = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;
        ptr_imgs.push(ptr);
    }

    let img_data = match read_image_data(cursor, &ptr_imgs, buffer_size) {
        Ok(data) => data,
        Err(e) => {
            println!("  - Warning: Failed to read image data: {:?}", e);
            println!("  - Using empty image data");
            Vec::new()
        }
    };

    if ptr_anim_info as u64 >= buffer_size - 16 {
        // Need at least 16 bytes for header
        println!("  - Warning: Animation info is missing or invalid");
        return Ok(WanFile {
            img_data,
            frame_data: Vec::new(),
            animation_groups: Vec::new(),
            body_part_offset_data: Vec::new(),
            custom_palette: palette_data,
            sdw_size: 1,
            wan_type: WanType::Character,
        });
    }

    // Read animation info
    cursor
        .seek(SeekFrom::Start(ptr_anim_info as u64))
        .map_err(|e| WanError::Io(e))?;

    let ptr_meta_frames_ref_table = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;
    let ptr_offsets_table = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;
    let ptr_anim_group_table = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;

    // Bounds check these pointers
    if ptr_meta_frames_ref_table as u64 >= buffer_size
        || ptr_offsets_table as u64 >= buffer_size
        || ptr_anim_group_table as u64 >= buffer_size
    {
        return Err(WanError::InvalidDataStructure(format!(
            "Animation pointers out of bounds. meta_frames: {:#x}, offsets: {:#x}, anim_group: {:#x}, buffer size: {}",
            ptr_meta_frames_ref_table, ptr_offsets_table, ptr_anim_group_table, buffer_size
        )));
    }

    let anim_groups_num = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;

    // Skip unknown values (Unk#6 through Unk#10)
    for _ in 0..5 {
        read_u16_le(cursor).map_err(|e| WanError::Io(e))?;
    }

    // Read animation groups
    let (animation_groups, anim_sequences) = match read_animation_groups(
        cursor,
        ptr_anim_group_table as u64,
        anim_groups_num as usize,
    ) {
        Ok(result) => result,
        Err(e) => {
            println!("  - Warning: Failed to read animation groups: {:?}", e);
            (Vec::new(), Vec::new())
        }
    };

    // Read meta frames
    let meta_frames = match read_meta_frames(
        cursor,
        ptr_meta_frames_ref_table as u64,
        ptr_offsets_table as u64,
    ) {
        Ok(frames) => frames,
        Err(e) => {
            println!("  - Warning: Failed to read meta frames: {:?}", e);
            Vec::new()
        }
    };

    // Read offset data
    let offset_data = match read_offset_data(cursor, ptr_offsets_table as u64, meta_frames.len()) {
        Ok(offsets) => offsets,
        Err(e) => {
            println!("  - Warning: Failed to read offset data: {:?}", e);
            Vec::new()
        }
    };

    // Read animation sequences
    let animation_data =
        match read_animation_sequences(cursor, &animation_groups, &anim_sequences)
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
        body_part_offset_data: offset_data,
        custom_palette: palette_data,
        sdw_size: 1,
        wan_type: WanType::Character,
    })
}

/// Parse an effect WAN file
pub fn parse_effect_wan(
    cursor: &mut Cursor<&[u8]>,
    buffer_size: u64,
) -> Result<WanFile, WanError> {
    // Read WAN header
    let ptr_anim_info = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;
    let ptr_image_data_info = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;

    // Validate pointers
    if ptr_anim_info == 0 || ptr_image_data_info == 0 {
        return Err(WanError::InvalidDataStructure(
            "Null pointer in WAN header".to_string(),
        ));
    }

    // Should be 2 or 3 for effect sprites
    let img_type = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;
    if img_type != 2 && img_type != 3 {
        println!(
            "  - Warning: Effect WAN with unexpected imgType {}",
            img_type
        );
    }

    // Skip unknown value (Unk#12)
    read_u16_le(cursor).map_err(|e| WanError::Io(e))?;

    // Read image data info
    cursor
        .seek(SeekFrom::Start(ptr_image_data_info as u64))
        .map_err(|e| WanError::Io(e))?;

    let ptr_image_data_table = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;
    let ptr_palette_info = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;

    // Validate pointers
    if ptr_image_data_table == 0 || ptr_palette_info == 0 {
        return Err(WanError::InvalidDataStructure(
            "Null pointer in Image Data Info".to_string(),
        ));
    }

    // Effect WAN may use 256 colours
    read_u16_le(cursor).map_err(|e| WanError::Io(e))?; // Unk#13 - ALWAYS 1
    let is_256_colour = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;
    read_u16_le(cursor).map_err(|e| WanError::Io(e))?; // Unk#11

    let img_num = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;

    // Read palette info
    cursor
        .seek(SeekFrom::Start(ptr_palette_info as u64))
        .map_err(|e| WanError::Io(e))?;

    let ptr_palette_data_block = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;

    // Unk#3 - Usually 1 except for effect_0001 - 0
    read_u16_le(cursor).map_err(|e| WanError::Io(e))?;

    // Total colours - but may not include all colours in the block
    let _total_colours = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;

    // Unk#4 - ALWAYS 1 except for effect_0001 - 0
    read_u16_le(cursor).map_err(|e| WanError::Io(e))?;

    // Unk#5 - palette offset, ALWAYS 269 except for effect_0001 and effect_0262 - 255
    let _palette_offset = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;

    // Read palette data for Effect WAN
    let palette_data = match read_effect_palette_data(
        cursor,
        ptr_palette_data_block as u64,
        ptr_palette_info as u64,
        is_256_colour as usize,
    ) {
        Ok(data) => data,
        Err(e) => {
            println!("  - Warning: Failed to read effect palette data: {:?}", e);
            vec![vec![(0, 0, 0, 0); 16]]
        }
    };

    // Read image data table
    cursor
        .seek(SeekFrom::Start(ptr_image_data_table as u64))
        .map_err(|e| WanError::Io(e))?;

    // Read pointers to image data
    let mut ptr_imgs = Vec::with_capacity(img_num as usize);
    for _ in 0..img_num {
        let ptr = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;
        ptr_imgs.push(ptr);
    }

    // Determine if we use the imgType 3 handling
    let img_data = if img_type == 3 {
        match read_effect_imgtype3_data(
            cursor,
            &ptr_imgs[0..1],
            buffer_size,
            is_256_colour as usize,
        ) {
            Ok(data) => data,
            Err(e) => {
                println!("  - Warning: Failed to read effect imgType3 data: {:?}", e);
                Vec::new()
            }
        }
    } else {
        match read_image_data(cursor, &ptr_imgs, buffer_size) {
            Ok(data) => data,
            Err(e) => {
                println!("  - Warning: Failed to read effect image data: {:?}", e);
                Vec::new()
            }
        }
    };

    // Some effect WAN files don't have animation data
    if ptr_anim_info == 0 {
        return Ok(WanFile {
            img_data,
            frame_data: Vec::new(),
            animation_groups: Vec::new(),
            body_part_offset_data: Vec::new(),
            custom_palette: palette_data,
            sdw_size: 1,
            wan_type: WanType::Effect,
        });
    }

    cursor
        .seek(SeekFrom::Start(ptr_anim_info as u64))
        .map_err(|e| WanError::Io(e))?;

    let ptr_meta_frames_ref_table = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;

    let _ptr_offsets_table = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;

    let ptr_anim_group_table = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;
    let nb_anim_groups = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;

    // Read animation groups
    let (animation_groups, anim_sequences) = match read_animation_groups(
        cursor,
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

    // Read effect meta frames
    let meta_frames = match read_effect_meta_frames(
        cursor,
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

    // Read animation sequences
    let animation_data =
        match read_animation_sequences(cursor, &animation_groups, &anim_sequences)
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

    Ok(WanFile {
        img_data,
        frame_data,
        animation_groups: animation_data,
        body_part_offset_data: offset_data,
        custom_palette: palette_data,
        sdw_size: 1,
        wan_type: WanType::Effect,
    })
}

/// Read palette data from the WAN file
fn read_palette_data(
    cursor: &mut Cursor<&[u8]>,
    ptr_palette_data_block: u64,
    end_ptr: u64,
    nb_colours_per_row: usize,
) -> Result<Vec<Vec<(u8, u8, u8, u8)>>, WanError> {
    debug_assert!(
        ptr_palette_data_block > 0,
        "Palette data block pointer is zero"
    );
    debug_assert!(end_ptr > 0, "End pointer is zero");
    debug_assert!(
        end_ptr > ptr_palette_data_block,
        "Invalid palette block range"
    );

    let _buffer_size = cursor.get_ref().len() as u64;

    // Seek to palette data block
    cursor
        .seek(SeekFrom::Start(ptr_palette_data_block))
        .map_err(|e| {
            println!("ERROR: Failed to seek to palette data block");
            WanError::Io(e)
        })?;

    let total_colours = ((end_ptr - ptr_palette_data_block) / 4) as usize;

    let total_palettes = if nb_colours_per_row == 0 {
        0
    } else {
        total_colours / nb_colours_per_row
    };

    let mut custom_palette = Vec::with_capacity(total_palettes);

    for _ in 0..total_palettes {
        let mut palette = Vec::with_capacity(nb_colours_per_row);

        for _ in 0..nb_colours_per_row {
            // Read colours in SkyTemple order - red, blue, green
            let red = read_u8(cursor).map_err(|e| {
                println!("ERROR: Failed to read red component");
                WanError::Io(e)
            })?;

            let blue = read_u8(cursor).map_err(|e| {
                println!("ERROR: Failed to read blue component");
                WanError::Io(e)
            })?;

            let green = read_u8(cursor).map_err(|e| {
                println!("ERROR: Failed to read green component");
                WanError::Io(e)
            })?;

            // Skip alpha byte
            let _ = read_u8(cursor).map_err(|e| {
                println!("ERROR: Failed to read alpha component");
                WanError::Io(e)
            })?;

            palette.push((red, blue, green, 255));
        }

        // Always ensure index 0 is transparent and we have 16 colours
        ensure_complete_palette(&mut palette);

        custom_palette.push(palette);
    }

    if custom_palette.is_empty() {
        println!("  No palettes found, creating default palette");
        let mut default_palette = vec![(0, 0, 0, 0)];
        ensure_complete_palette(&mut default_palette);
        custom_palette.push(default_palette);
    }

    return Ok(custom_palette);
}

/// Ensure a palette has all 16 colours, without modifying existing colours
fn ensure_complete_palette(palette: &mut Vec<(u8, u8, u8, u8)>) {
    if palette.is_empty() {
        palette.push((0, 0, 0, 0));
    }

    // If palette has less than 16 colours, pad with better defaults
    if palette.len() < 16 {
        let default_colours = [
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

        let needed = 16 - palette.len();
        for i in 0..needed.min(default_colours.len()) {
            palette.push(default_colours[i]);
        }

        while palette.len() < 16 {
            let val = ((palette.len() as u8) * 16).min(255);
            palette.push((val, val, val, 255));
        }
    }

    while palette.len() > 16 {
        palette.pop();
    }
}

/// Read palette data for Effect WAN with special handling for 256-colour mode
fn read_effect_palette_data(
    cursor: &mut Cursor<&[u8]>,
    ptr_palette_data_block: u64,
    ptr_palette_info: u64,
    is_256_colour: usize,
) -> Result<Vec<Vec<(u8, u8, u8, u8)>>, WanError> {
    cursor
        .seek(SeekFrom::Start(ptr_palette_data_block))
        .map_err(|e| WanError::Io(e))?;

    let total_bytes = ptr_palette_info - ptr_palette_data_block;
    let mut custom_palette = Vec::new();

    if is_256_colour == 4 {
        // Special case seen in effect267
        let colours_per_row_num = 256;
        let mut palette = vec![(0, 0, 0, 0); colours_per_row_num];

        let total_colours = total_bytes / 4;
        for jj in 0..total_colours as usize {
            let red = read_u8(cursor).map_err(|e| WanError::Io(e))? / 8 * 8 * 32 / 31;
            let blue = read_u8(cursor).map_err(|e| WanError::Io(e))? / 8 * 8 * 32 / 31;
            let green = read_u8(cursor).map_err(|e| WanError::Io(e))? / 8 * 8 * 32 / 31;
            read_u8(cursor).map_err(|e| WanError::Io(e))?; // Skip alpha

            if 16 + jj < colours_per_row_num {
                palette[16 + jj] = (red, blue, green, 255);
            }
        }
        custom_palette.push(palette);
    } else if is_256_colour == 1 {
        // 8bpp = 2^8 colours
        let colour_per_row_num = 256;
        let reads_per_row_num = 16;
        let total_colours = (total_bytes / 4) as usize;
        let total_palettes = total_colours / reads_per_row_num;

        for _ in 0..total_palettes {
            let mut palette = vec![(0, 0, 0, 0); colour_per_row_num];
            for jj in 0..reads_per_row_num {
                let red = read_u8(cursor).map_err(|e| WanError::Io(e))? / 8 * 8 * 32 / 31;
                let blue = read_u8(cursor).map_err(|e| WanError::Io(e))? / 8 * 8 * 32 / 31;
                let green = read_u8(cursor).map_err(|e| WanError::Io(e))? / 8 * 8 * 32 / 31;
                read_u8(cursor).map_err(|e| WanError::Io(e))?; // Skip alpha

                palette[16 + jj] = (red, blue, green, 255);
            }
            custom_palette.push(palette);
        }
    } else {
        // 4bpp = 2^4 colours
        let colours_per_row_num = 16;
        let total_colours = (total_bytes / 4) as usize;
        let total_palettes = total_colours / colours_per_row_num;

        for _ in 0..total_palettes {
            let mut palette = Vec::with_capacity(colours_per_row_num);
            for _ in 0..colours_per_row_num {
                let red = read_u8(cursor).map_err(|e| WanError::Io(e))?;
                let blue = read_u8(cursor).map_err(|e| WanError::Io(e))?;
                let green = read_u8(cursor).map_err(|e| WanError::Io(e))?;
                read_u8(cursor).map_err(|e| WanError::Io(e))?; // Skip alpha

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
    cursor: &mut Cursor<&[u8]>,
    ptr_imgs: &[u32],
    _buffer_size: u64,
) -> Result<Vec<ImgPiece>, WanError> {
    let mut img_data = Vec::with_capacity(ptr_imgs.len());

    for (img_idx, &ptr_img) in ptr_imgs.iter().enumerate() {
        if let Err(e) = cursor.seek(SeekFrom::Start(ptr_img as u64)) {
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

        // Read image data sections
        loop {
            // Read header values
            let ptr_pix_src = match read_u32_le(cursor) {
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

            let num_pixels_to_read = match read_u16_le(cursor) {
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
            if ptr_pix_src == 0 && num_pixels_to_read == 0 {
                break;
            }

            // Skip Unk#14
            if let Err(e) = read_u16_le(cursor) {
                println!(
                    "  - Warning: Failed to read unknown field for image #{}: {}",
                    img_idx, e
                );
                break;
            }

            img_piece.z_sort = match read_u32_le(cursor) {
                Ok(val) => val,
                Err(e) => {
                    println!(
                        "  - Warning: Failed to read z-sort value for image #{}: {}",
                        img_idx, e
                    );
                    0
                }
            };

            // Handle pixels
            let mut px_strip = Vec::with_capacity(num_pixels_to_read as usize);
            let mut pixels_read_in_strip = 0;

            if ptr_pix_src == 0 {
                // Zero padding case - only when pixel source is zero
                for _ in 0..num_pixels_to_read {
                    px_strip.push(0);
                    pixels_read_in_strip += 1;
                }
                valid_data = true;
            } else {
                let current_pos = cursor.position();

                // Use pixel source pointer directly
                if let Err(_) = cursor.seek(SeekFrom::Start(ptr_pix_src as u64)) {
                    if let Err(seek_e) = cursor.seek(SeekFrom::Start(current_pos)) {
                        println!("  - Warning: Failed to restore position: {}", seek_e);
                    }
                    continue;
                }
                
                for _ in 0..num_pixels_to_read {
                    match read_u8(cursor) {
                        Ok(px) => {
                            px_strip.push(px);
                            pixels_read_in_strip += 1;
                            valid_data = true;
                        }
                        Err(e) => {
                            println!(
                                "  - Warning: Partial read for image #{} at position {}: {} (collected {} of {} pixels)",
                                img_idx, 
                                cursor.position(), 
                                e,
                                pixels_read_in_strip,
                                num_pixels_to_read
                            );
                            break;
                        }
                    }
                }

                // Return to section position
                if let Err(e) = cursor.seek(SeekFrom::Start(current_pos)) {
                    println!("  - Warning: Failed to restore position after reading pixels for image #{}: {}", 
                             img_idx, e);
                    break;
                }
            }

            if !px_strip.is_empty() {
                img_piece.img_px.push(px_strip);
            }
        }

        if valid_data && !img_piece.img_px.is_empty() {
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
    Ok(img_data)
}

/// Read image data for Effect WAN with imgType 3
fn read_effect_imgtype3_data(
    cursor: &mut Cursor<&[u8]>,
    ptr_imgs: &[u32],
    _buffer_size: u64,
    is_256_colour: usize,
) -> Result<Vec<ImgPiece>, WanError> {
    let mut img_data = Vec::new();

    cursor
        .seek(SeekFrom::Start(ptr_imgs[0] as u64))
        .map_err(|e| WanError::Io(e))?;

    let mut img_piece = ImgPiece {
        img_px: Vec::new(),
        z_sort: 0,
    };

    let ptr_pix_src = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;
    let _atlas_width = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;
    let _atlas_height = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;
    img_piece.z_sort = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;

    cursor
        .seek(SeekFrom::Start(ptr_pix_src as u64))
        .map_err(|e| WanError::Io(e))?;

    let mut px_strip = Vec::new();
    while cursor.position() < ptr_imgs[0] as u64 {
        let px = read_u8(cursor).map_err(|e| WanError::Io(e))?;

        if is_256_colour == 0 {
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
    cursor: &mut Cursor<&[u8]>,
    ptr_meta_frames_ref_table: u64,
    ptr_frames_ref_table_end: u64,
) -> Result<Vec<MetaFrame>, WanError> {
    cursor
        .seek(SeekFrom::Start(ptr_meta_frames_ref_table))
        .map_err(|e| WanError::Io(e))?;

    // Read pointers to meta frames
    let mut ptr_meta_frames = Vec::new();
    while cursor.position() < ptr_frames_ref_table_end {
        let ptr = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;
        ptr_meta_frames.push(ptr);
    }

    let mut meta_frames = Vec::with_capacity(ptr_meta_frames.len());
    let _buffer_size = cursor.get_ref().len() as u64;

    for (_, &ptr_meta_frame) in ptr_meta_frames.iter().enumerate() {
        match cursor.seek(SeekFrom::Start(ptr_meta_frame as u64)) {
            Ok(_) => {},
            Err(e) => {
                println!("  ERROR: Failed to seek to frame position 0x{:x}: {}", ptr_meta_frame, e);
                return Err(WanError::Io(e));
            }
        }

        let mut meta_frame_pieces = Vec::new();

        loop {
            let current_piece_start_pos = cursor.position();
            
            // Read img_index with error handling
            let img_index = match read_i16_le(cursor) {
                Ok(val) => { 
                    val 
                },
                Err(e) => { 
                    println!("    ERROR Reading img_index at position 0x{:x}: {}", 
                             current_piece_start_pos, e); 
                    return Err(WanError::Io(e));
                }
            };
            
            // Read other attributes with error handling
            let _unk0 = match read_u16_le(cursor) {
                Ok(v) => v,
                Err(e) => {
                    println!("    ERROR Reading unk0 at position 0x{:x}: {}", 
                             cursor.position(), e);
                    return Err(WanError::Io(e));
                }
            };
            
            let attr0 = match read_u16_le(cursor) {
                Ok(v) => v,
                Err(e) => {
                    println!("    ERROR Reading attr0 at position 0x{:x}: {}", 
                             cursor.position(), e);
                    return Err(WanError::Io(e));
                }
            };
            
            let attr1 = match read_u16_le(cursor) {
                Ok(v) => v,
                Err(e) => {
                    println!("    ERROR Reading attr1 at position 0x{:x}: {}", 
                             cursor.position(), e);
                    return Err(WanError::Io(e));
                }
            };
            
            let attr2 = match read_u16_le(cursor) {
                Ok(v) => v,
                Err(e) => {
                    println!("    ERROR Reading attr2 at position 0x{:x}: {}", 
                             cursor.position(), e);
                    return Err(WanError::Io(e));
                }
            };

            let is_last = (attr1 & super::flags::ATTR1_IS_LAST_MASK) != 0;
            
            meta_frame_pieces.push(MetaFramePiece::new(img_index, attr0, attr1, attr2));

            if is_last {
                break;
            }
        }

        meta_frames.push(MetaFrame {
            pieces: meta_frame_pieces,
        });
    }

    Ok(meta_frames)
}

/// Read effect meta frames
fn read_effect_meta_frames(
    cursor: &mut Cursor<&[u8]>,
    ptr_meta_frames_ref_table: u64,
    ptr_anim_seq_table: u64,
) -> Result<Vec<MetaFrame>, WanError> {
    cursor
        .seek(SeekFrom::Start(ptr_meta_frames_ref_table))
        .map_err(|e| WanError::Io(e))?;

    // Read pointers to meta frames
    let mut ptr_meta_frames = Vec::new();
    while cursor.position() < ptr_anim_seq_table {
        let ptr = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;
        ptr_meta_frames.push(ptr);
    }

    // Read meta frames
    let mut meta_frames = Vec::with_capacity(ptr_meta_frames.len());
    let _buffer_size = cursor.get_ref().len() as u64;

    for (_, &ptr_meta_frame) in ptr_meta_frames.iter().enumerate() {
        cursor
            .seek(SeekFrom::Start(ptr_meta_frame as u64))
            .map_err(|e| WanError::Io(e))?;

        let mut meta_frame_pieces = Vec::new();

        loop {
            // Read first 2 bytes - should be FFFF
            let magic = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;
            if magic != 0xFFFF {
                // Not a valid metaframe, or we've reached the end
                break;
            }

            // Read section 1 - should be 0000
            let _section1 = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;

            // Read section 2 - 00 or FB (draw behind character)
            let _draw_behind = read_u8(cursor).map_err(|e| WanError::Io(e))? == 0xFB;

            // Read section 3 - Y offset
            let y_offset_lower = read_u8(cursor).map_err(|e| WanError::Io(e))?;

            // Read section 4 - Flags including upper bits of Y offset
            let section4 = read_u8(cursor).map_err(|e| WanError::Io(e))?;
            // Cast to u16 before shifting to avoid overflow
            let y_offset_upper = ((section4 & 0x03) as u16) << 8;
            let y_offset = y_offset_lower as u16 | y_offset_upper;

            // Read section 5 - X offset
            let x_offset_lower = read_u8(cursor).map_err(|e| WanError::Io(e))?;

            // Read section 6 - Flags including size, flips, and upper bits of X offset
            let section6 = read_u8(cursor).map_err(|e| WanError::Io(e))?;
            let size_bits = section6 & 0x03;
            let flip_vertical = (section6 & 0x04) != 0;
            let flip_horizontal = (section6 & 0x08) != 0;
            let is_last = (section6 & 0x10) != 0;
            // Cast to u16 before shifting to avoid overflow
            let x_offset_upper = ((section6 & 0x80) as u16) << 1;
            let x_offset = x_offset_lower as u16 | x_offset_upper;

            // Read section 7 - Image offset
            let image_offset = read_u8(cursor).map_err(|e| WanError::Io(e))?;

            // Read section 8 - Palette index
            let palette_index = read_u8(cursor).map_err(|e| WanError::Io(e))?;

            // Read section 9 - Should be 0x0C
            let _section9 = read_u8(cursor).map_err(|e| WanError::Io(e))?;

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
    cursor: &mut Cursor<&[u8]>,
    ptr_offsets_table: u64,
    num_frames: usize,
) -> Result<Vec<FrameOffset>, WanError> {
    cursor
        .seek(SeekFrom::Start(ptr_offsets_table))
        .map_err(|e| WanError::Io(e))?;

    let mut offset_data = Vec::with_capacity(num_frames);

    for _ in 0..num_frames {
        let head_x = read_i16_le(cursor).map_err(|e| WanError::Io(e))?;
        let head_y = read_i16_le(cursor).map_err(|e| WanError::Io(e))?;

        let lhand_x = read_i16_le(cursor).map_err(|e| WanError::Io(e))?;
        let lhand_y = read_i16_le(cursor).map_err(|e| WanError::Io(e))?;

        let rhand_x = read_i16_le(cursor).map_err(|e| WanError::Io(e))?;
        let rhand_y = read_i16_le(cursor).map_err(|e| WanError::Io(e))?;

        let center_x = read_i16_le(cursor).map_err(|e| WanError::Io(e))?;
        let center_y = read_i16_le(cursor).map_err(|e| WanError::Io(e))?;

        offset_data.push(FrameOffset::new(
            (head_x, head_y),
            (lhand_x, lhand_y),
            (rhand_x, rhand_y),
            (center_x, center_y),
        ));
    }

    Ok(offset_data)
}

/// Read animation groups from the WAN file
fn read_animation_groups(
    cursor: &mut Cursor<&[u8]>,
    ptr_anim_group_table: u64,
    num_anim_groups: usize,
) -> Result<(Vec<Vec<u32>>, Vec<u32>), WanError> {
    cursor
        .seek(SeekFrom::Start(ptr_anim_group_table))
        .map_err(|e| WanError::Io(e))?;

    let mut anim_groups: Vec<Vec<u32>> = Vec::with_capacity(num_anim_groups);
    let mut anim_sequences: Vec<u32> = Vec::new();
    let buffer_size = cursor.get_ref().len() as u64;

    for _group_idx in 0..num_anim_groups {
        let anim_loc = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;
        let anim_length = read_u16_le(cursor).map_err(|e| WanError::Io(e))?;

        // Skip Unk#16
        read_u16_le(cursor).map_err(|e| WanError::Io(e))?;

        let current_pos = cursor.position();

        // Skip empty groups
        if anim_loc == 0 || anim_length == 0 || anim_loc as u64 >= buffer_size {
            anim_groups.push(Vec::new());
            continue;
        }

        cursor
            .seek(SeekFrom::Start(anim_loc as u64))
            .map_err(|e| WanError::Io(e))?;

        let mut anim_ptrs = Vec::with_capacity(anim_length as usize);

        // Read all animation pointers in this group
        for _dir_idx in 0..anim_length {
            let anim_ptr = read_u32_le(cursor).map_err(|e| WanError::Io(e))?;
            anim_ptrs.push(anim_ptr);
            anim_sequences.push(anim_ptr);
        }

        anim_groups.push(anim_ptrs);

        // Restore position
        cursor
            .seek(SeekFrom::Start(current_pos))
            .map_err(|e| WanError::Io(e))?;
    }

    Ok((anim_groups, anim_sequences))
}

/// Read animation sequences from the WAN file
fn read_animation_sequences(
    cursor: &mut Cursor<&[u8]>,
    animation_groups: &[Vec<u32>],
    _anim_sequences: &[u32],
) -> Result<Vec<Vec<Animation>>, WanError> {
    let _buffer_size = cursor.get_ref().len() as u64;
    let mut result_animation_groups: Vec<Vec<Animation>> = 
        Vec::with_capacity(animation_groups.len());

    for (_group_idx, group) in animation_groups.iter().enumerate() {
        // Skip empty groups but preserve structure with empty Vec
        if group.is_empty() {
            result_animation_groups.push(Vec::new());
            continue;
        }

        let mut result_group: Vec<Animation> = Vec::with_capacity(group.len());

        // Process each animation pointer in this group
        for (dir_idx, &ptr) in group.iter().enumerate() {
            if dir_idx > 0 && ptr == group[dir_idx - 1] {
                if !result_group.is_empty() {
                    result_group.push(result_group[dir_idx - 1].clone());
                    continue;
                }
            }

            if let Err(e) = cursor.seek(SeekFrom::Start(ptr as u64)) {
                println!(
                    "    Error seeking to animation sequence at {:#x}: {}",
                    ptr, e
                );
                // Add an empty animation as a placeholder
                result_group.push(Animation::empty());
                continue;
            }

            let mut sequence_frames = Vec::new();

            loop {
                let frame_dur = match read_u8(cursor) {
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
                    if let Err(e) = cursor.read_exact(&mut skip_buf) {
                        println!("    Warning: Error skipping end marker: {}", e);
                    }
                    break;
                }

                // Read rest of frame data
                let flag = match read_u8(cursor) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("    Error reading frame flag: {}", e);
                        break;
                    }
                };

                let frame_index = match read_u16_le(cursor) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("    Error reading frame index: {}", e);
                        break;
                    }
                };

                let spr_off_x = match read_i16_le(cursor) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("    Error reading sprite offset X: {}", e);
                        break;
                    }
                };

                let spr_off_y = match read_i16_le(cursor) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("    Error reading sprite offset Y: {}", e);
                        break;
                    }
                };

                let sdw_off_x = match read_i16_le(cursor) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("    Error reading shadow offset X: {}", e);
                        break;
                    }
                };

                let sdw_off_y = match read_i16_le(cursor) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("    Error reading shadow offset Y: {}", e);
                        break;
                    }
                };

                sequence_frames.push(SequenceFrame::new(
                    frame_index,
                    frame_dur,
                    flag,
                    (spr_off_x, spr_off_y),
                    (sdw_off_x, sdw_off_y),
                ));
            }

            result_group.push(Animation::new(sequence_frames));
        }

        result_animation_groups.push(result_group.clone());
    }

    Ok(result_animation_groups)
}
