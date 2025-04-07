use std::io;
use std::ops::Range;

const HEADER_LEN: usize = 16;

/// SIR0 is a wrapper format that contains pointers to the actual data.
/// It is commonly used to wrap other data formats in PMD EoS.
pub struct Sir0 {
    /// Pointer to the data entry point (offset from end of header)
    pub data_pointer: u32,
    /// The actual content data
    pub content: Vec<u8>,
    /// List of offsets to pointers in the content that need to be handled
    pub content_pointer_offsets: Vec<u32>,
}

impl Sir0 {
    /// Create a new SIR0 container from raw components
    pub fn new(content: Vec<u8>, pointer_offsets: Vec<u32>, data_pointer: Option<u32>) -> Self {
        Sir0 {
            data_pointer: data_pointer.unwrap_or(0),
            content,
            content_pointer_offsets: pointer_offsets,
        }
    }

    pub fn from_bytes(data: &[u8]) -> Result<Sir0, io::Error> {
        if data.len() < 16 || &data[0..4] != b"SIR0" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Not a valid SIR0 file (missing magic number)",
            ));
        }

        // Read header fields
        let data_pointer = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let pointer_offset_list_pointer =
            u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

        // Decode pointer offsets
        let pointer_offsets = decode_sir0_pointer_offsets(data, pointer_offset_list_pointer);

        // Create a mutable copy of the data for pointer adjustment
        let mut data_copy = data.to_vec();

        // Adjust all pointers in the data by subtracting HEADER_LEN
        for &offset in &pointer_offsets {
            if offset as usize + 4 <= data_copy.len() {
                let ptr_value = u32::from_le_bytes([
                    data_copy[offset as usize],
                    data_copy[offset as usize + 1],
                    data_copy[offset as usize + 2],
                    data_copy[offset as usize + 3],
                ]);

                // Adjust by subtracting header length
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
        // Skip them and adjust the rest by HEADER_LEN for content pointers
        // This exactly matches SkyTemple's behavior
        let content_pointer_offsets: Vec<u32> = pointer_offsets
            .iter()
            .skip(2) // Skip the first two pointers (in header)
            .map(|&offset| {
                // Adjust offset by HEADER_LEN
                offset.checked_sub(HEADER_LEN as u32).unwrap_or_else(|| {
                    println!(
                        "Warning: Offset 0x{:x} too small to subtract header",
                        offset
                    );
                    offset
                })
            })
            .collect();

        // Adjust data_pointer by HEADER_LEN as well
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
            content_pointer_offsets,
            data_pointer: adjusted_data_pointer,
        })
    }

    /// Serialize this SIR0 container to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        // Implementation unchanged...
        // Your existing serialization code here
        Vec::new() // Placeholder
    }

    /// Unwrap content data at the data pointer position (if specified)
    pub fn unwrap(&self) -> Range<usize> {
        if self.data_pointer > 0 {
            self.data_pointer as usize..self.content.len()
        } else {
            0..self.content.len()
        }
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

        // Terminator condition - no flag and value is zero
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

/// Encode a list of pointer offsets to SIR0 format
fn encode_sir0_pointer_offsets(offsets: &[u32]) -> Vec<u8> {
    let mut encoded = Vec::new();
    let mut offset_so_far = 0u32;

    for &offset in offsets {
        let offset_to_encode = offset - offset_so_far;
        offset_so_far = offset;

        if offset_to_encode == 0 {
            // Special case for zero
            encoded.push(0);
            continue;
        }

        // Convert offset to bytes using 7 bits per byte with continuation bit
        let mut remaining = offset_to_encode;
        let mut bytes = Vec::new();

        while remaining > 0 {
            let byte = (remaining & 0x7F) as u8;
            remaining >>= 7;
            bytes.push(byte);
        }

        // Reverse the bytes and set continuation bits
        for i in (0..bytes.len()).rev() {
            let byte = if i > 0 { bytes[i] | 0x80 } else { bytes[i] };
            encoded.push(byte);
        }
    }

    // Add terminator
    encoded.push(0);

    encoded
}
