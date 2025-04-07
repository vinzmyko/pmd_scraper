use crate::containers::{CompressionContainer, ContainerHandler};
use std::io::{self};

// At4pxContainer is a specialised compression container used for compressed image data for Pokémon portrait sprites. It uses a bespoke
// implementation of the PX algorithm to decompress the container into image data.

pub const AT4PX_CONTAINER_HEADER_SIZE: usize = 0x12;

const PX_MIN_MATCH_SEQLEN: usize = 3;
const PX_LOOKBACK_BUFFER_SIZE: usize = 4096;

#[derive(Debug)]
pub struct At4pxContainer {
    pub _magic: [u8; 5],
    pub _container_length: u16,
    pub control_flags_bytes: [u8; 9],
    pub decompressed_size: u16,
    pub compressed_data: Vec<u8>,
}

impl At4pxContainer {
    pub fn get_container_size_and_deserialise(
        data: &[u8],
    ) -> io::Result<(usize, Box<dyn CompressionContainer>)> {
        if data.len() < 7 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Data too short to read container size",
            ));
        }

        if !data.starts_with(b"AT4PX") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid magic number - not an AT4PX container",
            ));
        }

        let container_length = u16::from_le_bytes([data[5], data[6]]) as usize;

        if container_length > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Container length ({}) exceeds available data ({})",
                    container_length,
                    data.len()
                ),
            ));
        }

        // Now deserialise it since we know it's valid
        let container = Self::deserialise(data)?;

        Ok((container_length, container))
    }
}

impl ContainerHandler for At4pxContainer {
    fn magic_word() -> &'static [u8] {
        b"AT4PX"
    }

    // Converts raw binary data to At4pxContainer
    fn deserialise(data: &[u8]) -> io::Result<Box<dyn CompressionContainer>> {
        if data.len() < AT4PX_CONTAINER_HEADER_SIZE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Data too short"));
        }

        let mut magic_bytes = [0u8; 5];
        magic_bytes.copy_from_slice(&data[0..5]);

        if !Self::matches(data) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid magic number",
            ));
        }

        let container_length = u16::from_le_bytes([data[5], data[6]]);
        let mut control_flags_bytes = [0u8; 9];
        control_flags_bytes.copy_from_slice(&data[7..16]);
        let decompressed_size = u16::from_le_bytes([data[16], data[17]]);

        let compressed_size = container_length as usize - AT4PX_CONTAINER_HEADER_SIZE;
        let compressed_data = data
            [AT4PX_CONTAINER_HEADER_SIZE..AT4PX_CONTAINER_HEADER_SIZE + compressed_size]
            .to_vec();

        Ok(Box::new(At4pxContainer {
            _magic: magic_bytes,
            _container_length: container_length,
            control_flags_bytes,
            decompressed_size,
            compressed_data,
        }))
    }
}

impl CompressionContainer for At4pxContainer {
    // Decompress the At4pxContainer to be processed later as an indexed data image
    fn decompress(&self) -> Result<Vec<u8>, String> {
        // Tracks the current position of compressed data
        let mut pos = 0;
        let mut decompressed = Vec::with_capacity(self.decompressed_size as usize);

        // Use a lookup table to check if a specific bit is set to 1
        let mut bit_lookup = [0u8; 8];
        for bit_pos in 0..8 {
            bit_lookup[bit_pos] = 1 << (7 - bit_pos);
        }

        // Main decompression loop
        while pos < self.compressed_data.len() {
            let control_byte = self.compressed_data[pos];
            pos += 1;

            // Check if we're at the end of compressed data
            if pos >= self.compressed_data.len() {
                break;
            }

            // Process all 8 bits of the control byte efficiently
            for bit_pos in 0..8 {
                if pos >= self.compressed_data.len() {
                    break;
                }

                let ctrl_bit = (control_byte & bit_lookup[bit_pos]) != 0;

                // If control bit is 1: the next byte is copied as is, else needs special handling
                if ctrl_bit {
                    decompressed.push(self.compressed_data[pos]);
                    pos += 1;
                } else {
                    match handle_compressed_sequence(
                        pos,
                        &self.compressed_data,
                        &mut decompressed,
                        &self.control_flags_bytes,
                    ) {
                        Ok(new_pos) => pos = new_pos,
                        Err(e) => return Err(e),
                    };
                }
            }
        }

        Ok(decompressed)
    }
}

// Special handling based on if high nibble matches control flag
fn handle_compressed_sequence(
    mut pos: usize,
    data: &[u8],
    decompressed: &mut Vec<u8>,
    control_flags_bytes: &[u8],
) -> Result<usize, String> {
    if pos >= data.len() {
        return Err("Unexpected end of compressed data".to_string());
    }

    let next_byte = data[pos];
    pos += 1;

    let high_nibble = (next_byte >> 4) & 0xF;
    let low_nibble = next_byte & 0xF;

    // Check if high nibble is in control flags
    let mut is_flag_match = false;
    let mut flag_idx = 0;

    for (idx, &flag) in control_flags_bytes.iter().enumerate() {
        if flag == high_nibble {
            is_flag_match = true;
            flag_idx = idx;
            break;
        }
    }

    if is_flag_match {
        // Create byte pattern
        let (byte1, byte2) = compute_nibble_pattern(flag_idx, low_nibble);
        decompressed.push(byte1);
        decompressed.push(byte2);
        Ok(pos)
    } else {
        if pos >= data.len() {
            return Err("Unexpected end of compressed data".to_string());
        }

        let next_byte = data[pos];
        pos += 1;

        let copy_len = (high_nibble as usize) + PX_MIN_MATCH_SEQLEN;
        let back_offset = (PX_LOOKBACK_BUFFER_SIZE as i32
            - ((low_nibble as i32) << 8)
            - next_byte as i32) as isize;

        let current_pos = decompressed.len();
        if back_offset as usize > current_pos {
            return Err(format!("Invalid back offset: {}", back_offset));
        }

        let start_pos = current_pos - back_offset as usize;

        for i in 0..copy_len {
            let src_pos = start_pos + (i % back_offset as usize);
            if src_pos >= decompressed.len() {
                return Err(format!("Invalid source position {}", src_pos));
            }
            let byte = decompressed[src_pos];
            decompressed.push(byte);
        }

        Ok(pos)
    }
}

fn compute_nibble_pattern(flag_idx: usize, low_nibble: u8) -> (u8, u8) {
    if flag_idx == 0 {
        let value = (low_nibble << 4) | low_nibble;
        return (value, value);
    }

    let mut nibble_base = low_nibble;

    match flag_idx {
        1 => nibble_base = nibble_base.wrapping_add(1),
        5 => nibble_base = nibble_base.wrapping_sub(1),
        _ => (),
    }

    let mut nibbles = [nibble_base; 4];

    match flag_idx {
        2..=4 => {
            nibbles[flag_idx - 1] = nibbles[flag_idx - 1].wrapping_sub(1);
        }
        6..=8 => {
            nibbles[flag_idx - 5] = nibbles[flag_idx - 5].wrapping_add(1);
        }
        _ => (),
    }

    (
        (nibbles[0] << 4) | nibbles[1],
        (nibbles[2] << 4) | nibbles[3],
    )
}
