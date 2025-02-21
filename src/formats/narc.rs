#[derive(Debug)]
#[allow(dead_code)]
pub struct NarcHeader {
    pub magic: [u8; 4],
    pub file_size: u32,
    pub chunk_size: u32,
    pub chunk_count: u32,
    pub byte_order: u16,
    pub version: u16,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct FatbHeader {
    pub magic: [u8; 4],
    pub size: u32,
    pub file_count: u16,
    pub reserved: u16,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct NarcFile {
    pub header: NarcHeader,
    pub fatb: FatbHeader,
    pub file_entries: Vec<(u32, u32)>,
    pub data: Vec<u8>,
}
