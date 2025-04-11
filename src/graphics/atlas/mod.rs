//! Sprite Atlas Generation
//!
//! This module provides functionality for creating sprite atlases from frame collections,
//! optimizing memory usage and rendering performance in game engines.

use crate::graphics::wan::{WanError, WanFile};
use image::{ColorType, ImageError, RgbaImage};
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use oxipng::{self, InFile, Options as OxiOptions, OutFile};

pub mod analyser;
pub mod generator;
pub mod metadata;

// Re-export key types and functions for external use
pub use analyser::{analyse_frames, calculate_optimal_size, AnalysedFrame, FrameAnalysis};
pub use generator::{
    create_atlas_layout, deduplicate_frames, generate_atlas, prepare_frames, AtlasLayout,
};
pub use metadata::{
    generate_metadata, save_metadata, AnimationInfo, AtlasMetadata, DirectionInfo, FrameInfo,
};

/// Configuration options for atlas generation
#[derive(Debug, Clone)]
pub struct AtlasConfig {
    /// Padding around frames (in pixels) - safety margin for offsets
    pub offset_padding: u32,
    /// Minimum frame width (for very small Pokémon)
    pub min_frame_width: u32,
    /// Minimum frame height
    pub min_frame_height: u32,
    /// Whether to deduplicate identical frames in the final atlas
    pub deduplicate_frames: bool,
    /// Whether to optimize final PNG compression (lossless, can be slow)
    pub optimise_compression: bool,
    /// Enable saving of intermediate debug images and files
    pub debug: bool,
    pub use_indexed_colour: bool,
    pub use_4bit_depth: bool,
}

impl Default for AtlasConfig {
    fn default() -> Self {
        Self {
            offset_padding: 4,
            min_frame_width: 32,
            min_frame_height: 32,
            deduplicate_frames: true,
            optimise_compression: true,
            debug: false,
            use_indexed_colour: true,
            use_4bit_depth: true,
        }
    }
}

/// The final result of the atlas generation process
#[derive(Debug)]
pub struct AtlasResult {
    /// The generated atlas image
    pub atlas_image: RgbaImage,
    /// The generated metadata describing the atlas content
    pub metadata: AtlasMetadata,
    /// Dimensions of the generated atlas image (width, height)
    pub dimensions: (u32, u32),
    /// Dimensions of each individual frame within the atlas (width, height)
    pub frame_dimensions: (u32, u32),
    /// Path where the atlas image was saved
    pub image_path: PathBuf,
    /// Path where the metadata JSON was saved
    pub metadata_path: PathBuf,
}

/// Error types specific to atlas operations
#[derive(Debug)]
pub enum AtlasError {
    Io(io::Error),
    Image(ImageError),
    Wan(WanError),
    Json(serde_json::Error),
    NoFramesFound,
    NoWanFilesProvided,
    AnalysisFailed(String),
    MetadataError(String),
    ConfigError(String),
}

// --- Error Conversions ---
impl From<io::Error> for AtlasError {
    fn from(err: io::Error) -> Self {
        AtlasError::Io(err)
    }
}
impl From<ImageError> for AtlasError {
    fn from(err: ImageError) -> Self {
        AtlasError::Image(err)
    }
}
impl From<WanError> for AtlasError {
    fn from(err: WanError) -> Self {
        AtlasError::Wan(err)
    }
}
impl From<serde_json::Error> for AtlasError {
    fn from(err: serde_json::Error) -> Self {
        AtlasError::Json(err)
    }
}

impl std::fmt::Display for AtlasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AtlasError::Io(err) => write!(f, "I/O error: {}", err),
            AtlasError::Image(err) => write!(f, "Image error: {}", err),
            AtlasError::Wan(err) => write!(f, "WAN error: {}", err),
            AtlasError::Json(err) => write!(f, "JSON error: {}", err),
            AtlasError::NoFramesFound => write!(f, "No valid frames found for atlas generation"),
            AtlasError::NoWanFilesProvided => write!(f, "No WAN files provided for processing"),
            AtlasError::AnalysisFailed(msg) => write!(f, "Frame analysis failed: {}", msg),
            AtlasError::MetadataError(msg) => write!(f, "Metadata generation error: {}", msg),
            AtlasError::ConfigError(msg) => write!(f, "Invalid configuration: {}", msg),
        }
    }
}
impl std::error::Error for AtlasError {}

// --- Main Atlas Creation Function ---

/// Creates a sprite atlas and associated metadata for a single Pokémon.
///
/// This function orchestrates the analysis, layout, generation, and metadata creation
/// based on the provided WAN files and configuration.
pub fn create_pokemon_atlas(
    wan_files: &HashMap<String, WanFile>,
    pokemon_id: usize, // ID from monster.md
    dex_num: u16,
    config: &AtlasConfig,
    output_dir: &Path,
) -> Result<AtlasResult, AtlasError> {
    if wan_files.is_empty() {
        return Err(AtlasError::NoWanFilesProvided);
    }

    let pokemon_name = if pokemon_id as u16 == dex_num {
        format!("pokemon_{:03}", dex_num)
    } else {
        let pokemon_form_num = pokemon_id as u16 - dex_num;
        format!("pokemon_{:03}_{:02}", dex_num, pokemon_form_num)
    };

    let pokemon_dir = output_dir.join(&pokemon_name);
    fs::create_dir_all(&pokemon_dir)?;

    // --- Step 1: Analyse Frames ---
    println!(
        "Analyzing frames for Pokémon #{:03} (Dex #{:03})...",
        pokemon_id, dex_num
    );
    let mut frame_analysis = analyser::analyse_frames(wan_files, pokemon_id, dex_num, config)?;
    if frame_analysis.ordered_frames.is_empty() {
        return Err(AtlasError::NoFramesFound);
    }
    println!(
        "  Analysis complete: {} original frames found.",
        frame_analysis.total_original_frames
    );

    // --- Step 2: Calculate Optimal Frame Size ---
    let (frame_width, frame_height) = analyser::calculate_optimal_size(&frame_analysis, config);
    println!(
        "  Optimal frame size calculated: {}x{}",
        frame_width, frame_height
    );

    // --- Step 3: Prepare Frames for Atlas ---
    let prepared_frames =
        generator::prepare_frames(&mut frame_analysis, frame_width, frame_height)?;
    println!("  Prepared {} frames for atlas.", prepared_frames.len());

    // --- Step 4: Deduplicate Frames (Optional) ---
    let (unique_frames, frame_mapping) = if config.deduplicate_frames {
        println!("  Deduplicating frames...");
        let (unique, mapping) = generator::deduplicate_frames(&prepared_frames);
        println!(
            "  Deduplication result: {} unique frames (reduced from {}).",
            unique.len(),
            prepared_frames.len()
        );
        (unique, mapping)
    } else {
        (
            prepared_frames,
            (0..frame_analysis.total_original_frames).collect(),
        )
    };

    // --- Step 5: Create Atlas Layout ---
    let atlas_layout =
        generator::create_atlas_layout(unique_frames.len(), frame_width, frame_height);
    println!(
        "  Atlas layout created: {}x{} grid, {}x{} total pixels.",
        atlas_layout.frames_per_row,
        atlas_layout.rows,
        atlas_layout.dimensions.0,
        atlas_layout.dimensions.1
    );

    // --- Step 6: Generate Atlas Image ---
    println!("  Generating atlas image...");
    let atlas_image = generator::generate_atlas(&unique_frames, &atlas_layout)?;

    // --- Step 7: Generate Metadata ---
    println!("  Generating metadata...");
    let shadow_size = get_shadow_size(wan_files); // Get shadow size from any input WAN
    let metadata = metadata::generate_metadata(
        wan_files,
        &frame_analysis, // Pass the analysis result containing all needed info
        frame_width,
        frame_height,
        &atlas_layout,
        &frame_mapping, // Pass the mapping from original index -> unique index
        shadow_size,
    )?;

    // --- Step 8: Save Results ---
    let atlas_filename = format!("{:03}_atlas.png", dex_num);
    let atlas_path = pokemon_dir.join(&atlas_filename);
    let metadata_filename = format!("{:03}_atlas.json", dex_num);
    let metadata_path = pokemon_dir.join(&metadata_filename);

    println!("  Saving atlas image to {}...", atlas_path.display());
    // Original code: atlas_image.save(&atlas_path)?;

    if config.use_indexed_colour {
        // Try indexed colour first
        if let Err(e) = save_indexed_atlas(&atlas_image, &atlas_path, config) {
            println!("  Warning: Failed to save with indexed palette: {}", e);
            atlas_image.save(&atlas_path)?;
        }
    } else {
        // Use standard RGBA save
        atlas_image.save(&atlas_path)?;
    }

    println!("  Saving metadata to {}...", metadata_path.display());
    metadata::save_metadata(&metadata, &metadata_path)?;

    // --- Save Debug Frames ---
    if config.debug {
        println!("  Saving debug frames...");
        let debug_dir = pokemon_dir.join("debug_unique_frames");
        fs::create_dir_all(&debug_dir)?;
        for (i, frame) in unique_frames.iter().enumerate() {
            let frame_path = debug_dir.join(format!("unique_frame_{:04}.png", i));
            frame.save(&frame_path)?;
        }
        println!(
            "  Saved {} unique frames to {}",
            unique_frames.len(),
            debug_dir.display()
        );
    }

    println!(
        "Successfully generated atlas for Pokémon #{:03}.",
        pokemon_id
    );

    Ok(AtlasResult {
        atlas_image,
        metadata,
        dimensions: atlas_layout.dimensions,
        frame_dimensions: (frame_width, frame_height),
        image_path: atlas_path,
        metadata_path,
    })
}

/// Save an atlas image using indexed colour for smaller file size
/// Can optionally optimise to 4-bit depth for maximum compression
pub fn save_indexed_atlas(
    atlas_image: &RgbaImage,
    path: &Path,
    config: &AtlasConfig,
) -> Result<(), AtlasError> {
    // First save the atlas image at full quality
    let temp_path = path.with_extension("temp.png");
    atlas_image
        .save(&temp_path)
        .map_err(|e| AtlasError::Image(e))?;

    if config.use_4bit_depth {
        // Use oxipng to optimize the PNG with appropriate settings
        let preset = if config.optimise_compression { 6 } else { 2 };

        // Create options using presets (much simpler API)
        let mut options = oxipng::Options::from_preset(preset);

        // Enable bit depth reduction for 4-bit output
        options.bit_depth_reduction = true;

        // Convert paths to PathBuf, which is what oxipng expects
        let in_path = temp_path.to_path_buf();
        let out_path = path.to_path_buf();

        // Optimize using InFile and OutFile with PathBuf values
        oxipng::optimize(
            &oxipng::InFile::Path(in_path),
            &oxipng::OutFile::Path(Some(out_path)),
            &options,
        )
        .map_err(|e| AtlasError::MetadataError(format!("PNG optimisation failed: {}", e)))?;

        // Remove temporary file
        if let Err(e) = std::fs::remove_file(&temp_path) {
            println!("  Warning: Failed to remove temporary file: {}", e);
        }
    } else {
        // Without 4-bit depth, just rename the temp file to the target
        if let Err(e) = std::fs::rename(&temp_path, path) {
            println!("  Warning: Failed to rename file: {}", e);
            // Try copying as fallback
            if let Err(e) = std::fs::copy(&temp_path, path) {
                return Err(AtlasError::Io(e));
            }
            // Remove the original if copy succeeded
            let _ = std::fs::remove_file(&temp_path);
        }
    }

    Ok(())
}

/// Convert an RGBA image to indexed colour format
fn convert_to_indexed(
    image: &RgbaImage,
    max_colours: usize,
) -> Result<(Vec<(u8, u8, u8, u8)>, Vec<u8>), AtlasError> {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let pixels = image.as_raw();

    // Collection of unique colours (RGBA)
    let mut unique_colours: Vec<(u8, u8, u8, u8)> = Vec::new();

    // Map position in image to colour index
    let mut indexed_data = vec![0u8; width * height];

    // Process each pixel
    for y in 0..height {
        for x in 0..width {
            let pos = (y * width + x) * 4;
            let r = pixels[pos];
            let g = pixels[pos + 1];
            let b = pixels[pos + 2];
            let a = pixels[pos + 3];

            // Skip fully transparent pixels (always index 0)
            if a == 0 {
                indexed_data[y * width + x] = 0;
                continue;
            }

            // Find if this colour exists in our palette
            let colour = (r, g, b, a);
            if let Some(idx) = unique_colours.iter().position(|&c| c == colour) {
                indexed_data[y * width + x] = idx as u8;
            } else {
                // Add new colour if we haven't reached max
                if unique_colours.len() < max_colours - 1 {
                    // Reserve index 0 for transparent
                    unique_colours.push(colour);
                    indexed_data[y * width + x] = unique_colours.len() as u8;
                } else {
                    // Find closest colour if we're at max
                    let closest_idx = find_closest_colour(&colour, &unique_colours);
                    indexed_data[y * width + x] = closest_idx as u8;
                }
            }
        }
    }

    // Ensure transparent colour is at index 0
    if !unique_colours.is_empty() && unique_colours[0].3 != 0 {
        // Add transparent colour at beginning
        unique_colours.insert(0, (0, 0, 0, 0));

        // Shift all indices by 1
        for idx in &mut indexed_data {
            *idx += 1;
        }
    } else if unique_colours.is_empty() {
        unique_colours.push((0, 0, 0, 0));
    }

    // If we have fewer colours than max, pad the palette
    while unique_colours.len() < max_colours {
        unique_colours.push((0, 0, 0, 0));
    }

    Ok((unique_colours, indexed_data))
}

/// Find the index of the closest colour in the palette
fn find_closest_colour(colour: &(u8, u8, u8, u8), palette: &[(u8, u8, u8, u8)]) -> usize {
    let mut best_idx = 0;
    let mut best_distance = u32::MAX;

    for (idx, pal_colour) in palette.iter().enumerate() {
        // Skip transparent colour for matching
        if pal_colour.3 == 0 {
            continue;
        }

        // Skip if alpha values are very different
        if (colour.3 as i32 - pal_colour.3 as i32).abs() > 128 {
            continue;
        }

        // Calculate colour distance (simple RGB distance)
        let dist = colour_distance(colour, pal_colour);

        if dist < best_distance {
            best_distance = dist;
            best_idx = idx;
        }
    }

    best_idx
}

/// Calculate colour distance between two RGBA colors
fn colour_distance(a: &(u8, u8, u8, u8), b: &(u8, u8, u8, u8)) -> u32 {
    let dr = (a.0 as i32 - b.0 as i32).abs() as u32;
    let dg = (a.1 as i32 - b.1 as i32).abs() as u32;
    let db = (a.2 as i32 - b.2 as i32).abs() as u32;
    let da = (a.3 as i32 - b.3 as i32).abs() as u32;

    // Weighted distance giving more importance to alpha
    dr * dr + dg * dg + db * db + da * da * 3
}

/// Helper to get shadow size from the first available WAN file.
fn get_shadow_size(wan_files: &HashMap<String, WanFile>) -> u8 {
    wan_files
        .values()
        .next()
        .map(|wan| wan.sdw_size)
        .unwrap_or(1) // Default to medium if no WAN files provided (shouldn't happen here)
}
