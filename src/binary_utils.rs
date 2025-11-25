use std::io::{self, Cursor, Read, Seek};

pub fn read_u8(cursor: &mut Cursor<&[u8]>) -> io::Result<u8> {
    if cursor.position() >= cursor.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "End of buffer reached",
        ));
    }

    let mut buf = [0u8; 1];
    cursor.read_exact(&mut buf)?;
    Ok(buf[0])
}

pub fn read_i8(cursor: &mut Cursor<&[u8]>) -> io::Result<i8> {
    if cursor.position() >= cursor.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "End of buffer reached",
        ));
    }

    let mut buf = [0u8; 1];
    cursor.read_exact(&mut buf)?;
    Ok(i8::from_le_bytes(buf))
}

pub fn read_u16_le(cursor: &mut Cursor<&[u8]>) -> io::Result<u16> {
    if cursor.position() + 1 >= cursor.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "End of buffer reached or not enough bytes for u16",
        ));
    }

    let mut buf = [0u8; 2];
    cursor.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

pub fn read_i16_le(cursor: &mut Cursor<&[u8]>) -> io::Result<i16> {
    if cursor.position() + 1 >= cursor.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "End of buffer reached or not enough bytes for i16",
        ));
    }

    let mut buf = [0u8; 2];
    cursor.read_exact(&mut buf)?;
    Ok(i16::from_le_bytes(buf))
}

pub fn read_u32_le(cursor: &mut Cursor<&[u8]>) -> io::Result<u32> {
    if cursor.position() + 3 >= cursor.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "End of buffer reached or not enough bytes for u32",
        ));
    }

    let mut buf = [0u8; 4];
    cursor.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

pub fn read_i32_le(cursor: &mut Cursor<&[u8]>) -> io::Result<i32> {
    if cursor.position() + 3 >= cursor.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "End of buffer reached or not enough bytes for i32",
        ));
    }

    let mut buf = [0u8; 4];
    cursor.read_exact(&mut buf)?;
    Ok(i32::from_le_bytes(buf))
}

pub fn seek_to(cursor: &mut Cursor<&[u8]>, position: u64) -> io::Result<()> {
    use std::io::SeekFrom;

    if position > cursor.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Cannot seek to position {} (buffer length: {})",
                position,
                cursor.get_ref().len()
            ),
        ));
    }

    cursor.seek(SeekFrom::Start(position))?;
    Ok(())
}

pub fn read_bytes(cursor: &mut Cursor<&[u8]>, length: usize) -> io::Result<Vec<u8>> {
    if cursor.position() + (length as u64) > cursor.get_ref().len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("Not enough bytes remaining for read_bytes({})", length),
        ));
    }

    let mut buffer = vec![0u8; length];
    cursor.read_exact(&mut buffer)?;
    Ok(buffer)
}

pub fn write_u32(data: &mut [u8], value: u32, pos: usize) {
    if pos + 4 <= data.len() {
        data[pos] = (value & 0xFF) as u8;
        data[pos + 1] = ((value >> 8) & 0xFF) as u8;
        data[pos + 2] = ((value >> 16) & 0xFF) as u8;
        data[pos + 3] = ((value >> 24) & 0xFF) as u8;
    }
}
