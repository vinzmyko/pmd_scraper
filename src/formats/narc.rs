// Nitro ARChive is Nintendo's archieve format used in DS games.
#[derive(Debug)]
#[allow(dead_code)]
pub struct NarcHeader {
    pub magic: [u8; 4],   // Always "NARC"
    pub file_size: u32,   // Total size == 4 bytes
    pub chunk_size: u16,  // Size header, always 0x0010
    pub chunk_count: u16, // Number of chunks, always 3
    pub byte_order: u16,  // Always 0xFFFE little-endian
    pub version: u16,
}

// Similar to File Allocation Table
#[derive(Debug)]
#[allow(dead_code)]
pub struct FatbHeader {
    pub magic: [u8; 4],  // Always "BTAF"
    pub size: u32,       // 4 bytes
    pub file_count: u16, // Number of files in archive
    pub reserved: u16,   // Always 0, 2 bytes
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct NarcFile {
    pub header: NarcHeader,
    pub fatb: FatbHeader,
    pub file_entries: Vec<(u32, u32)>, // Stores start and end pairs per file
    pub data: Vec<u8>,
}

#[allow(dead_code)]
impl NarcFile {
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < 16 || &data[0..4] != b"NARC" {
            return Err("Not a valid NARC file".to_string());
        }

        // Parse NARC header
        let header = NarcHeader {
            magic: [data[0], data[1], data[2], data[3]],
            byte_order: u16::from_le_bytes([data[4], data[5]]),
            version: u16::from_le_bytes([data[6], data[7]]),
            file_size: u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
            chunk_size: u16::from_le_bytes([data[12], data[13]]),
            chunk_count: u16::from_le_bytes([data[14], data[15]]),
        };

        // Locate the BTAF chunk
        let mut offset = 16; // After NARC header

        // Find BTAF chunk
        while offset + 8 <= data.len() {
            if &data[offset..offset + 4] == b"BTAF" {
                break;
            }
            offset += 4;
        }

        if offset + 8 > data.len() || &data[offset..offset + 4] != b"BTAF" {
            return Err("BTAF chunk not found".to_string());
        }

        // Parse BTAF header
        let fatb = FatbHeader {
            magic: [
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ],
            size: u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]),
            file_count: u16::from_le_bytes([data[offset + 8], data[offset + 9]]),
            reserved: u16::from_le_bytes([data[offset + 10], data[offset + 11]]),
        };

        // Move offset to start of file entries
        offset += 12;

        // Read file entries
        let mut file_entries = Vec::with_capacity(fatb.file_count as usize);
        for _ in 0..fatb.file_count {
            if offset + 8 > data.len() {
                return Err("Unexpected end of NARC data".to_string());
            }

            let start = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);

            let end = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            file_entries.push((start, end));
            offset += 8;
        }

        // Find the GMIF chunk (where the actual file data begins)
        let mut gmif_offset = offset;
        while gmif_offset + 4 <= data.len() {
            if &data[gmif_offset..gmif_offset + 4] == b"GMIF" {
                gmif_offset += 8; // Skip over GMIF header and size
                break;
            }
            gmif_offset += 4;
        }

        if gmif_offset >= data.len() {
            return Err("GMIF chunk not found".to_string());
        }

        Ok(NarcFile {
            header,
            fatb,
            file_entries,
            data: data.to_vec(), // Store a copy of the entire NARC data
        })
    }

    pub fn get_file(&self, index: usize) -> Option<&[u8]> {
        if index >= self.file_entries.len() {
            return None;
        }

        let (start, end) = self.file_entries[index];

        // Find the GMIF chunk offset
        let mut gmif_offset = 0;
        for i in 0..self.data.len() - 4 {
            if &self.data[i..i + 4] == b"GMIF" {
                gmif_offset = i + 8; // Skip GMIF header and size
                break;
            }
        }

        if gmif_offset == 0 {
            return None;
        }

        // Calculate the absolute offsets
        let abs_start = gmif_offset as u32 + start;
        let abs_end = gmif_offset as u32 + end;

        if abs_end as usize > self.data.len() {
            return None;
        }

        Some(&self.data[abs_start as usize..abs_end as usize])
    }
}
