use super::containers::{CompressionContainer, ContainerHandler, HEADER_SIZE};
use std::io::{self};

const PX_MIN_MATCH_SEQLEN: usize = 3;
const PX_LOOKBACK_BUFFER_SIZE: usize = 4096;

#[derive(Debug)]
pub struct At4pxContainer {
    pub magic: [u8; 5],
    pub container_length: u16,
    pub control_flags: [u8; 9],
    pub decompressed_size: u16,
    pub compressed_data: Vec<u8>,
}

impl At4pxContainer {
    pub fn get_container_size_and_deserialize(data: &[u8]) -> io::Result<(usize, Box<dyn CompressionContainer>)> {
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

        // Now deserialize it since we know it's valid
        let container = Self::deserialize(data)?;
        
        Ok((container_length, container))
    }
}

impl ContainerHandler for At4pxContainer {
    fn magic_word() -> &'static [u8] {
        b"AT4PX"
    }

    fn deserialize(data: &[u8]) -> io::Result<Box<dyn CompressionContainer>> {
        if data.len() < HEADER_SIZE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Data too short"));
        }

        let mut magic = [0u8; 5];
        magic.copy_from_slice(&data[0..5]);

        if !Self::matches(data) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid magic number",
            ));
        }

        let container_length = u16::from_le_bytes([data[5], data[6]]);
        let mut control_flags = [0u8; 9];
        control_flags.copy_from_slice(&data[7..16]);
        let decompressed_size = u16::from_le_bytes([data[16], data[17]]);

        let compressed_size = container_length as usize - HEADER_SIZE;
        let compressed_data = data[HEADER_SIZE..HEADER_SIZE + compressed_size].to_vec();

        Ok(Box::new(At4pxContainer {
            magic,
            container_length,
            control_flags,
            decompressed_size,
            compressed_data,
        }))
    }
}

impl CompressionContainer for At4pxContainer {
    fn decompress(&self) -> Result<Vec<u8>, String> {
        let mut pos = 0;
        let mut decompressed = Vec::with_capacity(800);

        while pos < self.compressed_data.len() {
            let control_byte = self.compressed_data[pos];
            pos += 1;

            for bit_pos in (0..8).rev() {
                if pos >= self.compressed_data.len() {
                    break;
                }

                let ctrl_bit = (control_byte & (1 << bit_pos)) != 0;

                if ctrl_bit {
                    decompressed.push(self.compressed_data[pos]);
                    pos += 1;
                } else {
                    pos = match handle_compressed_sequence(
                        pos,
                        &self.compressed_data,
                        &mut decompressed,
                        &self.control_flags,
                    ) {
                        Ok(new_pos) => new_pos,
                        Err(e) => return Err(e),
                    };
                }
            }
        }

        Ok(decompressed)
    }
}

fn handle_compressed_sequence(
    mut pos: usize,
    data: &[u8],
    decompressed: &mut Vec<u8>,
    control_flags: &[u8],
) -> Result<usize, String> {
    let next_byte = data[pos];
    pos += 1;

    let high_nibble = (next_byte >> 4) & 0xF;
    let low_nibble = next_byte & 0xF;

    let flag_idx = control_flags.iter().position(|&flag| flag == high_nibble);

    match flag_idx {
        Some(idx) => {
            let (byte1, byte2) = compute_nibble_pattern(idx, low_nibble);
            decompressed.push(byte1);
            decompressed.push(byte2);
            Ok(pos)
        }
        None => {
            if pos >= data.len() {
                return Err("Unexpected end of compressed data".to_string());
            }

            let next_byte = data[pos];
            pos += 1;

            let copy_len = (high_nibble as usize) + PX_MIN_MATCH_SEQLEN;
            let back_offset = (0x1000 - ((low_nibble as i32) << 8) - next_byte as i32) as isize;

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
