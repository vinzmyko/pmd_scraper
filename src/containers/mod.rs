pub mod binpack;
pub mod compression;
pub mod sir0;

use std::io;

pub trait CompressionContainer {
    fn decompress(&self) -> Result<Vec<u8>, String>;
}

pub trait ContainerHandler {
    fn magic_word() -> &'static [u8];
    fn matches(data: &[u8]) -> bool {
        data.starts_with(Self::magic_word())
    }
    fn deserialise(data: &[u8]) -> io::Result<Box<dyn CompressionContainer>>;
}
