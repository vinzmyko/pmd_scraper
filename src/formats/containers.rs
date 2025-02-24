use std::io;

pub const SUBENTRIES: usize = 40;
pub const SUBENTRY_LEN: usize = 4;
pub const KAO_IMG_PAL_SIZE: usize = 48;
pub const KAO_IMG_DIM: usize = 40;
pub const KAO_TILE_DIM: usize = 8;
pub const KAO_META_DIM: usize = 5;
pub const FIRST_TOC_OFFSET: usize = 160;
pub const HEADER_SIZE: usize = 0x12;

pub trait CompressionContainer {
    fn decompress(&self) -> Result<Vec<u8>, String>;
}

pub trait ContainerHandler {
    fn magic_word() -> &'static [u8];
    fn matches(data: &[u8]) -> bool {
        data.starts_with(Self::magic_word())
    }
    fn deserialize(data: &[u8]) -> io::Result<Box<dyn CompressionContainer>>;
}
