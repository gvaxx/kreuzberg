/// CFB stream reading and decompression utilities.
///
/// Consolidated from hwpers reader/cfb.rs, reader/stream.rs, and utils/compression.rs.
use super::error::{HwpError, Result};
use cfb::CompoundFile;
use flate2::read::{DeflateDecoder, ZlibDecoder};
use std::io::{Cursor, Read, Seek};

// ---------------------------------------------------------------------------
// CfbReader — opens a CFB compound file and reads named streams
// ---------------------------------------------------------------------------

pub struct CfbReader<F> {
    cfb: CompoundFile<F>,
}

impl CfbReader<Cursor<Vec<u8>>> {
    /// Open a CFB compound file from raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let cursor = Cursor::new(bytes.to_vec());
        let cfb = CompoundFile::open(cursor).map_err(|e| HwpError::Cfb(format!("Failed to open CFB: {e}")))?;
        Ok(Self { cfb })
    }
}

impl<F: Read + Seek> CfbReader<F> {
    /// Read a named stream into a `Vec<u8>`.
    pub fn read_stream(&mut self, path: &str) -> Result<Vec<u8>> {
        let mut stream = self
            .cfb
            .open_stream(path)
            .map_err(|e| HwpError::NotFound(format!("Stream '{path}' not found: {e}")))?;
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf)?;
        Ok(buf)
    }

    /// Return `true` if the named stream exists in the compound file.
    pub fn stream_exists(&self, path: &str) -> bool {
        self.cfb.exists(path)
    }
}

// ---------------------------------------------------------------------------
// StreamReader — cursor-backed little-endian binary reader
// ---------------------------------------------------------------------------

pub struct StreamReader {
    cursor: Cursor<Vec<u8>>,
}

impl StreamReader {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(data),
        }
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.cursor.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.cursor.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.cursor.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.cursor.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Current byte position within the stream.
    pub fn position(&self) -> u64 {
        self.cursor.position()
    }

    /// Number of bytes remaining from the current position to the end.
    pub fn remaining(&self) -> usize {
        let pos = self.cursor.position() as usize;
        let len = self.cursor.get_ref().len();
        len.saturating_sub(pos)
    }
}

// ---------------------------------------------------------------------------
// Decompression — HWP sections use raw deflate (with zlib fallback)
// ---------------------------------------------------------------------------

/// Decompress a raw-deflate stream from an HWP section.
///
/// HWP 5.0 compresses sections with raw deflate (no zlib header). Falls back
/// to zlib if raw deflate fails, and returns the data as-is if both fail.
///
/// Output is capped at `max_size` bytes to guard against decompression bombs;
/// exceeding the cap is an error rather than a silent truncation.
pub fn decompress_stream(data: &[u8], max_size: usize) -> Result<Vec<u8>> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    // Try raw deflate first (HWP standard)
    if let Some(decompressed) = read_limited(DeflateDecoder::new(data), max_size)? {
        return Ok(decompressed);
    }

    // Fall back to zlib
    if let Some(decompressed) = read_limited(ZlibDecoder::new(data), max_size)? {
        return Ok(decompressed);
    }

    // Return data unchanged (section may not actually be compressed)
    if data.len() > max_size {
        return Err(size_limit_error(max_size));
    }
    Ok(data.to_vec())
}

/// Read at most `max_size` bytes from `decoder`.
///
/// Returns `Ok(None)` when the stream is not valid for this decoder, and an
/// error when the decompressed output would exceed `max_size`.
fn read_limited<R: Read>(decoder: R, max_size: usize) -> Result<Option<Vec<u8>>> {
    let mut limited = decoder.take(max_size as u64 + 1);
    let mut decompressed = Vec::new();
    if limited.read_to_end(&mut decompressed).is_err() {
        return Ok(None);
    }
    if decompressed.len() > max_size {
        return Err(size_limit_error(max_size));
    }
    Ok(Some(decompressed))
}

fn size_limit_error(max_size: usize) -> HwpError {
    HwpError::CompressionError(format!("Decompressed HWP section exceeds {max_size} byte limit"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;

    fn deflate(data: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn test_decompress_stream_within_limit() {
        let compressed = deflate(&[b'a'; 1024]);
        assert_eq!(decompress_stream(&compressed, 1024).unwrap(), vec![b'a'; 1024]);
    }

    #[test]
    fn test_decompress_stream_rejects_bomb() {
        let compressed = deflate(&vec![0u8; 10 * 1024 * 1024]);
        assert!(compressed.len() < 64 * 1024);
        let err = decompress_stream(&compressed, 1024 * 1024).unwrap_err();
        assert!(matches!(err, HwpError::CompressionError(_)));
    }
}
