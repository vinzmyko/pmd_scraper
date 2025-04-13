// Common_AT is handled here
use crate::containers::{CompressionContainer, ContainerHandler};
use std::io::{self};

// PKDPX is a general-purpose compression container format
// Its data is compressed using the PX compression algorithm

pub const PKDPX_CONTAINER_HEADER_SIZE: usize = 0x14;
const PX_MIN_MATCH_SEQLEN: usize = 3;
const PX_LOOKBACK_BUFFER_SIZE: usize = 4096; // 0x1000

#[derive(Debug)]
pub struct PkdpxContainer {
    pub _magic: [u8; 5],
    pub _length_compressed: u16,
    pub compression_flags: [u8; 9],
    pub length_decompressed: u32,
    pub compressed_data: Vec<u8>,
}

impl ContainerHandler for PkdpxContainer {
    fn magic_word() -> &'static [u8] {
        b"PKDPX"
    }

    fn deserialise(data: &[u8]) -> io::Result<Box<dyn CompressionContainer>> {
        if data.len() < PKDPX_CONTAINER_HEADER_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Data too short for PKDPX header",
            ));
        }

        // Verify magic number
        let mut magic = [0u8; 5];
        magic.copy_from_slice(&data[0..5]);

        if !Self::matches(data) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid magic number (expected 'PKDPX')",
            ));
        }

        // Read header fields
        let length_compressed = u16::from_le_bytes([data[5], data[6]]);

        let mut compression_flags = [0u8; 9];
        compression_flags.copy_from_slice(&data[7..16]);

        let length_decompressed = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);

        // Get the compressed data (after header)
        let compressed_size = length_compressed as usize - PKDPX_CONTAINER_HEADER_SIZE;
        let compressed_data = data
            [PKDPX_CONTAINER_HEADER_SIZE..PKDPX_CONTAINER_HEADER_SIZE + compressed_size]
            .to_vec();

        Ok(Box::new(PkdpxContainer {
            _magic: magic,
            _length_compressed: length_compressed,
            compression_flags,
            length_decompressed,
            compressed_data,
        }))
    }
}

impl CompressionContainer for PkdpxContainer {
    fn decompress(&self) -> Result<Vec<u8>, String> {
        // Allocate output buffer based on decompressed size
        let mut decompressed = Vec::with_capacity(self.length_decompressed as usize);

        // Create a lookup table for bit positions
        let bit_masks = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];

        // Current position in the compressed data
        let mut data_pos = 0;

        // Main decompression loop
        while data_pos < self.compressed_data.len() {
            // Read control byte that determines how to interpret the next 8 operations
            let control_byte = self.compressed_data[data_pos];
            data_pos += 1;

            // Process each bit in the control byte
            for bit in 0..8 {
                if data_pos >= self.compressed_data.len() {
                    break;
                }

                let is_literal = (control_byte & bit_masks[bit]) != 0;

                if is_literal {
                    let literal_byte = self.compressed_data[data_pos];
                    decompressed.push(literal_byte);
                    data_pos += 1;
                } else {
                    if data_pos + 1 >= self.compressed_data.len() {
                        return Err("Unexpected end of compressed data".to_string());
                    }

                    let first_byte = self.compressed_data[data_pos];
                    data_pos += 1;

                    let high_nibble = first_byte >> 4;
                    let low_nibble = first_byte & 0x0F;

                    // Check if high nibble matches any compression flag
                    let mut is_flag = false;
                    let mut flag_idx = 0;

                    for (idx, &flag) in self.compression_flags.iter().enumerate() {
                        if flag == high_nibble {
                            is_flag = true;
                            flag_idx = idx;
                            break;
                        }
                    }

                    if is_flag {
                        // Handle special pattern case
                        let pattern = compute_nibble_pattern(flag_idx, low_nibble);
                        decompressed.push(pattern.0);
                        decompressed.push(pattern.1);
                    } else {
                        // Handle back-reference (this is where the bug might be)
                        let second_byte = self.compressed_data[data_pos];
                        data_pos += 1;

                        // Calculate copy length
                        let copy_len = (high_nibble as usize) + PX_MIN_MATCH_SEQLEN;

                        // Calculate back offset using correct formula
                        let back_offset = (PX_LOOKBACK_BUFFER_SIZE as i32
                            - ((low_nibble as i32) << 8)
                            - second_byte as i32)
                            as usize;

                        // Verify the offset is valid
                        if back_offset > decompressed.len() {
                            return Err(format!(
                                "Invalid back reference offset: {} (output size: {})",
                                back_offset,
                                decompressed.len()
                            ));
                        }
                        if back_offset == 0 {
                            return Err("Back reference offset cannot be zero".to_string());
                        }

                        // Calculate start position for copying
                        let start_pos = decompressed.len() - back_offset;

                        // Copy bytes from earlier in the output
                        for i in 0..copy_len {
                            // Use modulo to handle repeating patterns
                            let src_index = start_pos + (i % back_offset);

                            // Bounds check
                            if src_index >= decompressed.len() {
                                return Err(format!(
                                    "Invalid source index: {} >= {}",
                                    src_index,
                                    decompressed.len()
                                ));
                            }

                            let byte = decompressed[src_index];
                            decompressed.push(byte);
                        }
                    }
                }
            }

            // break if we've reached expected length
            if decompressed.len() >= self.length_decompressed as usize {
                break;
            }
        }

        if decompressed.len() != self.length_decompressed as usize {
            println!(
                "WARNING: Expected {} bytes but got {} bytes",
                self.length_decompressed,
                decompressed.len()
            );
        }

        Ok(decompressed)
    }
}

// Helper function to compute nibble patterns for special compression flags
fn compute_nibble_pattern(flag_idx: usize, low_nibble: u8) -> (u8, u8) {
    // Simple case - all four nibbles are the same (flag 0)
    if flag_idx == 0 {
        // In this case, all our 4 nibbles have the value of the "low_nibble" as their value
        // Since we're dealing with half bytes, shift one left by 4 and bitwise OR it with the other!
        let value = (low_nibble << 4) | low_nibble;
        return (value, value);
    }

    // For all other flags, start with base value for all nibbles
    let mut nibble_base = low_nibble;

    // Apply first modification: adjust the base value for all nibbles
    match flag_idx {
        1 => nibble_base = nibble_base.wrapping_add(1), // Flag 1: Add 1 to all nibbles
        5 => nibble_base = nibble_base.wrapping_sub(1), // Flag 5: Subtract 1 from all nibbles
        _ => (), // All other flags (0, 2-4, 6-8) leave base value unchanged
    }

    // Create array with the base value
    let mut nibbles = [nibble_base; 4];

    // Apply second modification: adjust specific nibble based on flag index
    match flag_idx {
        // Flags 1-4: Decrement the indexed nibble
        1..=4 => {
            nibbles[flag_idx - 1] = nibbles[flag_idx - 1].wrapping_sub(1);
        }
        // Flags 5-8: Increment the indexed nibble
        5..=8 => {
            nibbles[flag_idx - 5] = nibbles[flag_idx - 5].wrapping_add(1);
        }
        // Flag 0 has no second modification (already handled by the initial if)
        _ => (),
    }

    // Combine nibbles into two bytes
    let byte1 = (nibbles[0] << 4) | nibbles[1];
    let byte2 = (nibbles[2] << 4) | nibbles[3];

    (byte1, byte2)
}
