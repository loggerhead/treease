use std::io::{IoSliceMut, Read, Write};

use super::errors::CoreError;

pub fn write_byte<W: Write + ?Sized>(writer: &mut W, byte: u8) -> Result<(), CoreError> {
    writer.write_all(&[byte]).map_err(CoreError::from)
}

pub fn write_byte_n_times<W: Write + ?Sized>(
    writer: &mut W,
    byte: u8,
    count: usize,
) -> Result<(), CoreError> {
    const CHUNK_SIZE: usize = 256;
    let chunk = [byte; CHUNK_SIZE];
    let mut remaining = count;
    while remaining > 0 {
        let to_write = remaining.min(CHUNK_SIZE);
        writer.write_all(&chunk[..to_write])?;
        remaining -= to_write;
    }
    Ok(())
}

pub fn read_all<R: Read + ?Sized>(reader: &mut R) -> Result<Vec<u8>, CoreError> {
    let mut out = Vec::new();
    reader.read_to_end(&mut out)?;
    Ok(out)
}

pub struct AnyWriter<'a> {
    inner: &'a mut dyn Write,
}

impl<'a> AnyWriter<'a> {
    pub fn new<W: Write + 'a>(writer: &'a mut W) -> Self {
        Self { inner: writer }
    }

    pub fn write_all(&mut self, bytes: &[u8]) -> Result<(), CoreError> {
        self.inner.write_all(bytes).map_err(CoreError::from)
    }
}

pub struct AnyReader<'a> {
    inner: &'a mut dyn Read,
}

impl<'a> AnyReader<'a> {
    pub fn new<R: Read + 'a>(reader: &'a mut R) -> Self {
        Self { inner: reader }
    }

    pub fn read_all(&mut self) -> Result<Vec<u8>, CoreError> {
        const CHUNK_SIZE: usize = 8 * 1024;
        let mut out = Vec::new();

        loop {
            let mut chunk = [0_u8; CHUNK_SIZE];
            let mut bufs = [IoSliceMut::new(&mut chunk)];
            let read = self.inner.read_vectored(&mut bufs)?;
            if read == 0 {
                break;
            }
            out.extend_from_slice(&chunk[..read]);
        }

        Ok(out)
    }
}

impl Read for AnyReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> std::io::Result<usize> {
        self.inner.read_vectored(bufs)
    }
}

pub struct VecWriter<'a> {
    bytes: &'a mut Vec<u8>,
}

impl<'a> VecWriter<'a> {
    pub fn new(bytes: &'a mut Vec<u8>) -> Self {
        Self { bytes }
    }
}

pub fn array_list_writer(bytes: &mut Vec<u8>) -> VecWriter<'_> {
    VecWriter::new(bytes)
}

pub fn writer_from_pointer<W: Write>(writer: &mut W) -> AnyWriter<'_> {
    AnyWriter::new(writer)
}

pub fn reader_from_pointer<R: Read>(reader: &mut R) -> AnyReader<'_> {
    AnyReader::new(reader)
}

impl Write for VecWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
