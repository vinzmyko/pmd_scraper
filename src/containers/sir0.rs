use std::io;

const HEADER_LEN: usize = 16;

/// SIR0 is a wrapper format that contains pointers to the actual data.
pub struct Sir0 {
    /// Pointer to the data entry point (offset from end of header)
    pub data_pointer: u32,
    pub content: Vec<u8>,
    /// List of offsets to pointers in the content that need to be handled
    pub _content_pointer_offsets: Vec<u32>,
}

impl Sir0 {
    pub fn from_bytes(data: &[u8]) -> Result<Sir0, io::Error> {
        if data.len() < 16 || &data[0..4] != b"SIR0" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Not a valid SIR0 file (missing magic number)",
            ));
        }

        let data_pointer = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let pointer_offset_list_pointer =
            u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

        let pointer_offsets = decode_sir0_pointer_offsets(data, pointer_offset_list_pointer);

        let mut data_copy = data.to_vec();

        for &offset in &pointer_offsets {
            if offset as usize + 4 <= data_copy.len() {
                let ptr_value = u32::from_le_bytes([
                    data_copy[offset as usize],
                    data_copy[offset as usize + 1],
                    data_copy[offset as usize + 2],
                    data_copy[offset as usize + 3],
                ]);

                let adjusted_ptr = if ptr_value >= HEADER_LEN as u32 {
                    ptr_value - HEADER_LEN as u32
                } else {
                    println!(
                        "Warning: Pointer at offset 0x{:x} is too small to subtract header: 0x{:x}",
                        offset, ptr_value
                    );
                    ptr_value
                };

                // Write back adjusted pointer
                let adjusted_bytes = adjusted_ptr.to_le_bytes();
                data_copy[offset as usize] = adjusted_bytes[0];
                data_copy[offset as usize + 1] = adjusted_bytes[1];
                data_copy[offset as usize + 2] = adjusted_bytes[2];
                data_copy[offset as usize + 3] = adjusted_bytes[3];
            }
        }

        // The first two pointer offsets are for the header pointers
        let content_pointer_offsets: Vec<u32> = pointer_offsets
            .iter()
            .skip(2)
            .map(|&offset| {
                offset.checked_sub(HEADER_LEN as u32).unwrap_or_else(|| {
                    println!(
                        "Warning: Offset 0x{:x} too small to subtract header",
                        offset
                    );
                    offset
                })
            })
            .collect();

        let adjusted_data_pointer = if data_pointer >= HEADER_LEN as u32 {
            data_pointer - HEADER_LEN as u32
        } else {
            println!(
                "Warning: Data pointer 0x{:x} too small to subtract header",
                data_pointer
            );
            data_pointer
        };

        // Extract content data from data_copy
        let content_end = pointer_offset_list_pointer as usize;
        let content_start = HEADER_LEN as usize;

        if content_end <= content_start || content_end > data_copy.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid content range: start={}, end={}, data_len={}",
                    content_start,
                    content_end,
                    data_copy.len()
                ),
            ));
        }

        let content = data_copy[content_start..content_end].to_vec();

        Ok(Sir0 {
            content,
            _content_pointer_offsets: content_pointer_offsets,
            data_pointer: adjusted_data_pointer,
        })
    }
}

/// Decode SIR0 pointer offsets from the encoded format
pub fn decode_sir0_pointer_offsets(data: &[u8], pointer_offset_list_pointer: u32) -> Vec<u32> {
    let mut decoded = Vec::new();
    // This is used to sum up all offsets and obtain the offset relative to the file
    let mut offset_sum = 0u32;
    // Temp buffer to assemble longer offsets
    let mut buffer = 0u32;
    // This tracks whether the previous byte had the continuation bit flag
    let mut last_had_bit_flag = false;

    let mut pos = pointer_offset_list_pointer as usize;

    // Process until end of data or terminating zero
    while pos < data.len() {
        let cur_byte = data[pos];
        pos += 1;

        if !last_had_bit_flag && cur_byte == 0 {
            break;
        }

        // Ignore the first bit (0x80), using the 0x7F bitmask
        buffer |= (cur_byte & 0x7F) as u32;

        if (0x80 & cur_byte) != 0 {
            // Continuation bit set - shift and continue
            last_had_bit_flag = true;
            buffer <<= 7;
        } else {
            // End of sequence - add to offset sum and record
            last_had_bit_flag = false;
            offset_sum += buffer;
            decoded.push(offset_sum);

            buffer = 0;
        }
    }
    decoded
}
