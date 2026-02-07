//! Streaming ZIP reader for EPUB files
//!
//! Memory-efficient ZIP reader that streams files without loading entire archive.
//! Uses fixed-size central directory cache (max 256 entries, ~4KB).
//! Supports DEFLATE decompression using miniz_oxide.

extern crate alloc;

use alloc::string::{String, ToString};
use heapless::Vec as HeaplessVec;
use log;
use miniz_oxide::inflate::decompress_slice_iter_to_slice;
use std::io::{Read, Seek, SeekFrom};

/// Maximum number of central directory entries to cache
const MAX_CD_ENTRIES: usize = 256;

/// Maximum filename length in ZIP entries
const MAX_FILENAME_LEN: usize = 256;

/// Decompression buffer size (4KB)
const DECOMPRESS_BUF_SIZE: usize = 4096;

/// Local file header signature (little-endian)
const SIG_LOCAL_FILE_HEADER: u32 = 0x04034b50;

/// Central directory entry signature (little-endian)
const SIG_CD_ENTRY: u32 = 0x02014b50;

/// End of central directory signature (little-endian)
const SIG_EOCD: u32 = 0x06054b50;

/// Compression methods
const METHOD_STORED: u16 = 0;
const METHOD_DEFLATED: u16 = 8;

/// ZIP error types
#[derive(Debug, Clone, PartialEq)]
pub enum ZipError {
    /// File not found in archive
    FileNotFound,
    /// Invalid ZIP format
    InvalidFormat,
    /// Unsupported compression method
    UnsupportedCompression,
    /// Decompression failed
    DecompressError,
    /// CRC32 mismatch
    CrcMismatch,
    /// I/O error
    IoError,
    /// Central directory full
    CentralDirFull,
    /// Buffer too small
    BufferTooSmall,
}

/// Central directory entry metadata
#[derive(Debug, Clone)]
pub struct CdEntry {
    /// Compression method (0=stored, 8=deflated)
    pub method: u16,
    /// Compressed size in bytes
    pub compressed_size: u32,
    /// Uncompressed size in bytes
    pub uncompressed_size: u32,
    /// Offset to local file header
    pub local_header_offset: u32,
    /// CRC32 checksum
    pub crc32: u32,
    /// Filename (max 255 chars)
    pub filename: String,
}

impl CdEntry {
    /// Create new empty entry
    fn new() -> Self {
        Self {
            method: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            local_header_offset: 0,
            crc32: 0,
            filename: String::new(),
        }
    }
}

/// Streaming ZIP file reader
pub struct StreamingZip<F: Read + Seek> {
    /// File handle
    file: F,
    /// Central directory entries (fixed size)
    entries: HeaplessVec<CdEntry, MAX_CD_ENTRIES>,
    /// Number of entries in central directory
    num_entries: usize,
    /// Offset to central directory start
    cd_offset: u64,
}

impl<F: Read + Seek> StreamingZip<F> {
    /// Open a ZIP file and parse the central directory
    pub fn new(mut file: F) -> Result<Self, ZipError> {
        // Find and parse EOCD
        let (cd_offset, num_entries) = Self::find_eocd(&mut file)?;

        let mut entries: HeaplessVec<CdEntry, MAX_CD_ENTRIES> = HeaplessVec::new();

        // Parse central directory entries
        file.seek(SeekFrom::Start(cd_offset))
            .map_err(|_| ZipError::IoError)?;

        for _ in 0..num_entries.min(MAX_CD_ENTRIES as u16) {
            if let Some(entry) = Self::read_cd_entry(&mut file)? {
                entries.push(entry).map_err(|_| ZipError::CentralDirFull)?;
            }
        }

        log::info!(
            "[ZIP] Central directory at offset {}, expecting {} entries",
            cd_offset,
            num_entries
        );
        log::info!(
            "[ZIP] Successfully parsed {} entries into cache",
            entries.len()
        );

        // Debug: list all entries
        for (i, entry) in entries.iter().enumerate() {
            log::info!(
                "[ZIP] Entry[{}]: '{}' (offset={}, compressed={}, uncompressed={})",
                i,
                entry.filename,
                entry.local_header_offset,
                entry.compressed_size,
                entry.uncompressed_size
            );
        }

        Ok(Self {
            file,
            entries,
            num_entries: num_entries as usize,
            cd_offset,
        })
    }

    /// Find EOCD and extract central directory info
    fn find_eocd(file: &mut F) -> Result<(u64, u16), ZipError> {
        // Get file size
        let file_size = file.seek(SeekFrom::End(0)).map_err(|_| ZipError::IoError)?;

        if file_size < 22 {
            return Err(ZipError::InvalidFormat);
        }

        // Scan last 1KB for EOCD signature
        let scan_range = file_size.min(1024) as usize;
        let mut buffer = [0u8; 1024];

        file.seek(SeekFrom::Start(file_size - scan_range as u64))
            .map_err(|_| ZipError::IoError)?;
        let bytes_read = file
            .read(&mut buffer[..scan_range])
            .map_err(|_| ZipError::IoError)?;

        // Scan backwards for EOCD signature
        for i in (0..=bytes_read.saturating_sub(22)).rev() {
            if Self::read_u32_le(&buffer, i) == SIG_EOCD {
                // Found EOCD, extract info
                let num_entries = Self::read_u16_le(&buffer, i + 8);
                let cd_offset = Self::read_u32_le(&buffer, i + 16) as u64;
                return Ok((cd_offset, num_entries));
            }
        }

        Err(ZipError::InvalidFormat)
    }

    /// Read a central directory entry from file
    fn read_cd_entry(file: &mut F) -> Result<Option<CdEntry>, ZipError> {
        let mut sig_buf = [0u8; 4];
        if file.read_exact(&mut sig_buf).is_err() {
            return Ok(None);
        }
        let sig = u32::from_le_bytes(sig_buf);

        if sig != SIG_CD_ENTRY {
            return Ok(None); // End of central directory
        }

        // Read fixed portion of central directory entry (42 bytes = offsets 4-45)
        // This includes everything up to and including the local header offset
        let mut buf = [0u8; 42];
        file.read_exact(&mut buf).map_err(|_| ZipError::IoError)?;

        let mut entry = CdEntry::new();

        // Parse central directory entry fields
        // buf contains bytes 4-49 of the CD entry (after the 4-byte signature)
        // buf[N] corresponds to CD entry offset (N + 4)
        entry.method = u16::from_le_bytes([buf[6], buf[7]]); // CD offset 10
        entry.crc32 = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]); // CD offset 16
        entry.compressed_size = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]); // CD offset 20
        entry.uncompressed_size = u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]); // CD offset 24
        let name_len = u16::from_le_bytes([buf[24], buf[25]]) as usize; // CD offset 28
        let extra_len = u16::from_le_bytes([buf[26], buf[27]]) as usize; // CD offset 30
        let comment_len = u16::from_le_bytes([buf[28], buf[29]]) as usize; // CD offset 32
        entry.local_header_offset = u32::from_le_bytes([buf[38], buf[39], buf[40], buf[41]]); // CD offset 42

        // Read filename
        if name_len > 0 && name_len < MAX_FILENAME_LEN {
            let mut name_buf = alloc::vec![0u8; name_len];
            file.read_exact(&mut name_buf)
                .map_err(|_| ZipError::IoError)?;
            entry.filename = String::from_utf8_lossy(&name_buf).to_string();
        }

        // Skip extra field and comment
        let skip_bytes = extra_len + comment_len;
        if skip_bytes > 0 {
            file.seek(SeekFrom::Current(skip_bytes as i64))
                .map_err(|_| ZipError::IoError)?;
        }

        Ok(Some(entry))
    }

    /// Get entry by filename (case-insensitive)
    pub fn get_entry(&self, name: &str) -> Option<&CdEntry> {
        let name_lower = name.to_lowercase();
        self.entries
            .iter()
            .find(|e| e.filename.to_lowercase() == name_lower)
    }

    /// Debug: Log all entries in the ZIP (for troubleshooting)
    pub fn debug_list_entries(&self) {
        log::info!(
            "[ZIP] Central directory contains {} entries:",
            self.entries.len()
        );
        for (i, entry) in self.entries.iter().enumerate() {
            log::info!(
                "[ZIP]  [{}] '{}' (method={}, compressed={}, uncompressed={})",
                i,
                entry.filename,
                entry.method,
                entry.compressed_size,
                entry.uncompressed_size
            );
        }
    }

    /// Read and decompress a file into the provided buffer
    /// Returns number of bytes written to buffer
    pub fn read_file(&mut self, entry: &CdEntry, buf: &mut [u8]) -> Result<usize, ZipError> {
        if entry.uncompressed_size as usize > buf.len() {
            return Err(ZipError::BufferTooSmall);
        }

        // Calculate data offset by reading local file header
        let data_offset = self.calc_data_offset(entry)?;

        // Seek to data
        self.file
            .seek(SeekFrom::Start(data_offset))
            .map_err(|_| ZipError::IoError)?;

        match entry.method {
            METHOD_STORED => {
                // Read stored data directly
                let size = entry.compressed_size as usize;
                if size > buf.len() {
                    return Err(ZipError::BufferTooSmall);
                }
                self.file
                    .read_exact(&mut buf[..size])
                    .map_err(|_| ZipError::IoError)?;
                Ok(size)
            }
            METHOD_DEFLATED => {
                // Read compressed data and decompress
                let compressed_size = entry.compressed_size as usize;
                let mut compressed_buf = alloc::vec![0u8; compressed_size];
                self.file
                    .read_exact(&mut compressed_buf)
                    .map_err(|_| ZipError::IoError)?;

                // Decompress using miniz_oxide
                let result = decompress_slice_iter_to_slice(
                    buf,
                    core::iter::once(compressed_buf.as_slice()),
                    false, // zlib_header: ZIP uses raw deflate, not zlib
                    true,  // ignore_adler32
                );

                match result {
                    Ok(decompressed_len) => {
                        // Verify CRC32 if available
                        if entry.crc32 != 0 {
                            let calc_crc = crc32fast::hash(&buf[..decompressed_len]);
                            if calc_crc != entry.crc32 {
                                return Err(ZipError::CrcMismatch);
                            }
                        }
                        Ok(decompressed_len)
                    }
                    Err(_) => Err(ZipError::DecompressError),
                }
            }
            _ => Err(ZipError::UnsupportedCompression),
        }
    }

    /// Read a file by its local header offset and size (avoids borrow issues)
    /// This is useful when you need to read a file after getting its metadata
    pub fn read_file_at_offset(
        &mut self,
        local_header_offset: u32,
        _uncompressed_size: usize,
        buf: &mut [u8],
    ) -> Result<usize, ZipError> {
        // Find entry by offset
        let entry = self
            .entries
            .iter()
            .find(|e| e.local_header_offset == local_header_offset)
            .ok_or(ZipError::FileNotFound)?;

        // Create a temporary entry clone to avoid borrow issues
        let entry_clone = CdEntry {
            method: entry.method,
            compressed_size: entry.compressed_size,
            uncompressed_size: entry.uncompressed_size,
            local_header_offset: entry.local_header_offset,
            crc32: entry.crc32,
            filename: entry.filename.clone(),
        };

        self.read_file(&entry_clone, buf)
    }

    /// Calculate the offset to the actual file data (past local header)
    fn calc_data_offset(&mut self, entry: &CdEntry) -> Result<u64, ZipError> {
        let offset = entry.local_header_offset as u64;
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|_| ZipError::IoError)?;

        // Read local file header (30 bytes fixed + variable filename/extra)
        let mut header = [0u8; 30];
        self.file
            .read_exact(&mut header)
            .map_err(|_| ZipError::IoError)?;

        // Verify signature
        let sig = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        if sig != SIG_LOCAL_FILE_HEADER {
            return Err(ZipError::InvalidFormat);
        }

        // Get filename and extra field lengths
        let name_len = u16::from_le_bytes([header[26], header[27]]) as u64;
        let extra_len = u16::from_le_bytes([header[28], header[29]]) as u64;

        // Data starts after local header + filename + extra field
        let data_offset = offset + 30 + name_len + extra_len;

        Ok(data_offset)
    }

    /// Read u16 from buffer at offset (little-endian)
    fn read_u16_le(buf: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes([buf[offset], buf[offset + 1]])
    }

    /// Read u32 from buffer at offset (little-endian)
    fn read_u32_le(buf: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ])
    }

    /// Get number of entries in central directory
    pub fn num_entries(&self) -> usize {
        self.num_entries.min(self.entries.len())
    }

    /// Iterate over all entries
    pub fn entries(&self) -> impl Iterator<Item = &CdEntry> {
        self.entries.iter()
    }

    /// Get entry by index
    pub fn get_entry_by_index(&self, index: usize) -> Option<&CdEntry> {
        self.entries.get(index)
    }
}

/// Convenience function to open an EPUB file
pub fn open_epub<F: Read + Seek>(file: F) -> Result<StreamingZip<F>, ZipError> {
    StreamingZip::new(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple test to verify the module compiles
    #[test]
    fn test_zip_error_debug() {
        let err = ZipError::FileNotFound;
        assert_eq!(format!("{:?}", err), "FileNotFound");
    }
}
