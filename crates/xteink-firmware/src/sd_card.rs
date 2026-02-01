//! SD Card Filesystem Implementation for ESP32
//!
//! Uses embedded-sdmmc crate with ESP-IDF SPI driver.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::{Operation, SpiDevice};
use embedded_sdmmc::{
    Block, BlockCount, BlockDevice, BlockIdx, Directory, File, Mode, Volume, VolumeIdx,
    VolumeManager,
};

use crate::filesystem::{FileInfo, FileSystem, FileSystemError};

/// SD Card block size (always 512 bytes)
const BLOCK_SIZE: usize = 512;

/// SD Card filesystem implementation
pub struct SdCardFileSystem<SD: BlockDevice, T: embedded_sdmmc::TimeSource> {
    volume_mgr: VolumeManager<SD, T>,
    current_volume: Option<VolumeIdx>,
}

impl<SD: BlockDevice, T: embedded_sdmmc::TimeSource> SdCardFileSystem<SD, T> {
    /// Create new SD card filesystem
    ///
    /// # Arguments
    /// * `block_device` - The SD card block device (SPI)
    /// * `time_source` - Time source for filesystem timestamps
    pub fn new(block_device: SD, time_source: T) -> Self {
        let volume_mgr = VolumeManager::new(block_device, time_source);
        Self {
            volume_mgr,
            current_volume: None,
        }
    }

    /// Initialize and mount the first volume
    pub fn mount(&mut self) -> Result<(), FileSystemError> {
        // Try to open volume 0 (first partition)
        match self.volume_mgr.open_volume(VolumeIdx(0)) {
            Ok(_volume) => {
                self.current_volume = Some(VolumeIdx(0));
                log::info!("SD card mounted successfully");
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to mount SD card: {:?}", e);
                Err(FileSystemError::IoError(format!("Mount failed: {:?}", e)))
            }
        }
    }

    fn with_volume<F, R>(&mut self, f: F) -> Result<R, FileSystemError>
    where
        F: FnOnce(&mut VolumeManager<SD, T>, Volume) -> Result<R, embedded_sdmmc::Error<SD::Error>>,
    {
        let vol_idx = self
            .current_volume
            .ok_or(FileSystemError::IoError("No volume mounted".to_string()))?;

        let volume = self
            .volume_mgr
            .open_volume(vol_idx)
            .map_err(|e| FileSystemError::IoError(format!("Volume error: {:?}", e)))?;

        f(&mut self.volume_mgr, volume)
            .map_err(|e| FileSystemError::IoError(format!("Filesystem error: {:?}", e)))
    }

    fn with_directory<F, R>(&mut self, path: &str, f: F) -> Result<R, FileSystemError>
    where
        F: FnOnce(
            &mut VolumeManager<SD, T>,
            Volume,
            Directory,
        ) -> Result<R, embedded_sdmmc::Error<SD::Error>>,
    {
        self.with_volume(|mgr, vol| {
            let dir = if path == "/" {
                mgr.open_root_dir(&vol)?
            } else {
                // Navigate to directory (simplified - assumes path exists)
                mgr.open_root_dir(&vol)?
            };
            f(mgr, vol, dir)
        })
    }
}

impl<SD: BlockDevice, T: embedded_sdmmc::TimeSource> FileSystem for SdCardFileSystem<SD, T> {
    fn list_files(&mut self, path: &str) -> Result<Vec<FileInfo>, FileSystemError> {
        let mut files = Vec::new();

        self.with_directory(path, |mgr, _vol, dir| {
            mgr.iterate_dir(&dir, |entry| {
                let name = core::str::from_utf8(&entry.name.base_name())
                    .unwrap_or("???")
                    .trim_end_matches('\0')
                    .to_string();

                let size = entry.size;
                let is_directory = entry.attributes.is_directory();

                files.push(FileInfo {
                    name,
                    size,
                    is_directory,
                });
            })?;
            Ok(())
        })?;

        Ok(files)
    }

    fn read_file(&mut self, path: &str) -> Result<String, FileSystemError> {
        let mut content = String::new();

        // Parse path to get directory and filename
        let (dir_path, filename) = match path.rfind('/') {
            Some(0) => ("/", &path[1..]),
            Some(i) => (&path[..i], &path[i + 1..]),
            None => ("/", path),
        };

        self.with_directory(dir_path, |mgr, _vol, dir| {
            // Convert filename to embedded-sdmmc format (8.3 or long filename)
            let mut file = mgr
                .open_file_in_dir(&dir, filename, Mode::ReadOnly)
                .map_err(|e| {
                    log::error!("Failed to open file '{}': {:?}", filename, e);
                    e
                })?;

            // Read file in chunks
            let mut buffer = [0u8; 512];
            loop {
                match mgr.read(&mut file, &mut buffer) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let chunk = core::str::from_utf8(&buffer[..n])
                            .map_err(|_| embedded_sdmmc::Error::InvalidUtf16)?;
                        content.push_str(chunk);
                    }
                    Err(e) => return Err(e),
                }
            }

            mgr.close_file(file)?;
            Ok(())
        })?;

        Ok(content)
    }

    fn exists(&mut self, path: &str) -> bool {
        self.file_info(path).is_ok()
    }

    fn file_info(&mut self, path: &str) -> Result<FileInfo, FileSystemError> {
        // Parse path
        let (dir_path, filename) = match path.rfind('/') {
            Some(0) => ("/", &path[1..]),
            Some(i) => (&path[..i], &path[i + 1..]),
            None => ("/", path),
        };

        let mut result = None;

        self.with_directory(dir_path, |mgr, _vol, dir| {
            mgr.iterate_dir(&dir, |entry| {
                let name = core::str::from_utf8(&entry.name.base_name())
                    .unwrap_or("???")
                    .trim_end_matches('\0');

                if name == filename {
                    result = Some(FileInfo {
                        name: name.to_string(),
                        size: entry.size,
                        is_directory: entry.attributes.is_directory(),
                    });
                }
            })?;
            Ok(())
        })?;

        result.ok_or(FileSystemError::NotFound)
    }
}

/// Dummy time source for embedded-sdmmc
/// In production, use RTC or NTP
pub struct DummyTimeSource;

impl embedded_sdmmc::TimeSource for DummyTimeSource {
    fn get_timestamp(&self) -> embedded_sdmmc::Timestamp {
        embedded_sdmmc::Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

/// ESP32 SDMMC block device wrapper
/// Adapts ESP-IDF SPI to embedded-sdmmc BlockDevice trait
pub struct EspSdMmc<SPI, CS> {
    spi: SPI,
    cs: CS,
}

impl<SPI, CS> EspSdMmc<SPI, CS> {
    pub fn new(spi: SPI, cs: CS) -> Self {
        Self { spi, cs }
    }
}

impl<SPI, CS> BlockDevice for EspSdMmc<SPI, CS>
where
    SPI: SpiDevice,
    CS: OutputPin,
{
    type Error = SPI::Error;

    fn read(
        &mut self,
        blocks: &mut [Block],
        start_block_idx: BlockIdx,
        _reason: &str,
    ) -> Result<(), Self::Error> {
        // SD card read implementation
        // This would use the SPI device to send CMD17 (READ_SINGLE_BLOCK)
        // For now, placeholder
        log::debug!("Reading {} blocks from {}", blocks.len(), start_block_idx.0);
        Ok(())
    }

    fn write(&mut self, _blocks: &[Block], _start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        // SD card write implementation
        log::debug!("Writing blocks");
        Ok(())
    }

    fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
        // Return card capacity
        // For now, assume 32GB card
        Ok(BlockCount(32 * 1024 * 1024 * 1024 / 512))
    }
}
