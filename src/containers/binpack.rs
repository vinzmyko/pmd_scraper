// BinPack is a simple container format that stores multiple files
// with a header containing a table of contents
pub struct BinPack {
    files: Vec<Vec<u8>>,
}

impl BinPack {
    /// Deserialize a BinPack from bytes
    pub fn from_bytes(data: &[u8]) -> std::io::Result<Self> {
        if data.len() < 8 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Data too short for BinPack header",
            ));
        }

        // First 4 bytes are zero, next 4 bytes are file count
        let num_files = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        
        // Parse table of contents
        let mut files = Vec::with_capacity(num_files);
        for i in 0..num_files {
            let toc_offset = 8 + i * 8;
            if toc_offset + 8 > data.len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid TOC entry",
                ));
            }
            
            // Read pointer and length from TOC
            let ptr = u32::from_le_bytes([
                data[toc_offset], data[toc_offset+1], 
                data[toc_offset+2], data[toc_offset+3]
            ]) as usize;
            
            let len = u32::from_le_bytes([
                data[toc_offset+4], data[toc_offset+5], 
                data[toc_offset+6], data[toc_offset+7]
            ]) as usize;
            
            // Extract file data
            if ptr + len > data.len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("File data extends beyond bounds: {}+{}", ptr, len),
                ));
            }
            
            files.push(data[ptr..(ptr + len)].to_vec());
        }

        Ok(BinPack { files })
    }

    /// Serialize to bytes, with optional fixed header length
    pub fn to_bytes(&self, fixed_header_len: usize) -> Vec<u8> {
        // Calculate minimum header size
        let base_header_size = 8 + self.files.len() * 8;
        
        // Ensure header is 16-byte aligned and at least fixed_header_len
        let header_size = if fixed_header_len > 0 {
            fixed_header_len
        } else {
            if base_header_size % 16 != 0 {
                base_header_size + (16 - base_header_size % 16)
            } else {
                base_header_size
            }
        };
        
        // Initialize output buffer
        let mut output = vec![0u8; header_size];
        
        // Write file count
        output[4..8].copy_from_slice(&(self.files.len() as u32).to_le_bytes());
        
        // Padding bytes for header (per SkyTemple implementation)
        if header_size > base_header_size {
            for i in base_header_size..header_size {
                output[i] = 0xFF;
            }
        }
        
        // Write files and update TOC
        let mut current_pos = header_size;
        for (i, file) in self.files.iter().enumerate() {
            // Update TOC entry
            let toc_offset = 8 + i * 8;
            output[toc_offset..toc_offset+4].copy_from_slice(&(current_pos as u32).to_le_bytes());
            output[toc_offset+4..toc_offset+8].copy_from_slice(&(file.len() as u32).to_le_bytes());
            
            // Append file data
            output.extend_from_slice(file);
            
            // Add padding to align to 16 bytes
            let padding_needed = (16 - (output.len() % 16)) % 16;
            output.extend(vec![0xFF; padding_needed]);
            
            current_pos = output.len();
        }
        
        output
    }

    // Collection-like methods for easier usage
    
    pub fn get(&self, index: usize) -> Option<&[u8]> {
        self.files.get(index).map(|v| v.as_slice())
    }
    
    pub fn len(&self) -> usize {
        self.files.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
    
    pub fn append(&mut self, data: Vec<u8>) {
        self.files.push(data);
    }
}

// Allow direct indexing
impl std::ops::Index<usize> for BinPack {
    type Output = Vec<u8>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.files[index]
    }
}

impl std::ops::IndexMut<usize> for BinPack {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.files[index]
    }
}

// Iterator support
impl<'a> IntoIterator for &'a BinPack {
    type Item = &'a Vec<u8>;
    type IntoIter = std::slice::Iter<'a, Vec<u8>>;

    fn into_iter(self) -> Self::IntoIter {
        self.files.iter()
    }
}
