//! SD card filesystem using embedded-sdmmc

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use embedded_sdmmc::{sdcard::DummyCsPin, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager};
use esp_idf_svc::hal::delay::FreeRtos;
use xteink_ui::filesystem::{FileInfo, FileSystem, FileSystemError};

pub struct DummyTimeSource;

impl TimeSource for DummyTimeSource {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 56,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

pub struct SdCardFs<SPI>
where
    SPI: embedded_hal::spi::SpiDevice,
{
    volume_mgr: VolumeManager<SdCard<SPI, DummyCsPin, FreeRtos>, DummyTimeSource, 4, 4, 1>,
}

impl<SPI> SdCardFs<SPI>
where
    SPI: embedded_hal::spi::SpiDevice,
{
    pub fn new(spi: SPI) -> Result<Self, FileSystemError> {
        let sdcard = SdCard::new(spi, DummyCsPin, FreeRtos);

        log::info!("SD card size: {} bytes", sdcard.num_bytes().unwrap_or(0));

        let volume_mgr = VolumeManager::new(sdcard, DummyTimeSource);
        Ok(Self { volume_mgr })
    }
}

impl<SPI> FileSystem for SdCardFs<SPI>
where
    SPI: embedded_hal::spi::SpiDevice,
{
    fn list_files(&mut self, path: &str) -> Result<Vec<FileInfo>, FileSystemError> {
        let mut volume = self
            .volume_mgr
            .open_volume(VolumeIdx(0))
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

        let mut root_dir = volume
            .open_root_dir()
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

        let mut files = Vec::new();

        if path == "/" {
            root_dir
                .iterate_dir(|entry| {
                    let base = core::str::from_utf8(entry.name.base_name())
                        .unwrap_or("")
                        .trim_end();
                    let ext = core::str::from_utf8(entry.name.extension())
                        .unwrap_or("")
                        .trim_end();

                    let full_name = if ext.is_empty() {
                        base.to_string()
                    } else {
                        format!("{}.{}", base, ext)
                    };

                    if !full_name.is_empty() && !full_name.starts_with('.') {
                        files.push(FileInfo {
                            name: full_name.to_lowercase(),
                            size: entry.size as u64,
                            is_directory: entry.attributes.is_directory(),
                        });
                    }
                })
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        } else {
            let mut sub_dir = root_dir
                .open_dir(path.trim_start_matches('/'))
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

            sub_dir
                .iterate_dir(|entry| {
                    let base = core::str::from_utf8(entry.name.base_name())
                        .unwrap_or("")
                        .trim_end();
                    let ext = core::str::from_utf8(entry.name.extension())
                        .unwrap_or("")
                        .trim_end();

                    let full_name = if ext.is_empty() {
                        base.to_string()
                    } else {
                        format!("{}.{}", base, ext)
                    };

                    if !full_name.is_empty() && !full_name.starts_with('.') {
                        files.push(FileInfo {
                            name: full_name.to_lowercase(),
                            size: entry.size as u64,
                            is_directory: entry.attributes.is_directory(),
                        });
                    }
                })
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        }

        Ok(files)
    }

    fn read_file(&mut self, path: &str) -> Result<String, FileSystemError> {
        let mut volume = self
            .volume_mgr
            .open_volume(VolumeIdx(0))
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

        let mut root_dir = volume
            .open_root_dir()
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

        let filename = path.trim_start_matches('/');
        let mut file = root_dir
            .open_file_in_dir(filename, embedded_sdmmc::Mode::ReadOnly)
            .map_err(|_| FileSystemError::NotFound)?;

        let file_size = file.length() as usize;
        let mut buffer = Vec::with_capacity(file_size);
        let mut chunk = [0u8; 512];
        let mut total_read = 0usize;

        while total_read < file_size {
            let to_read = (file_size - total_read).min(chunk.len());
            match file.read(&mut chunk[..to_read]) {
                Ok(0) => break,
                Ok(n) => {
                    buffer.extend_from_slice(&chunk[..n]);
                    total_read += n;
                }
                Err(e) => {
                    return Err(FileSystemError::IoError(format!("{:?}", e)));
                }
            }
        }

        String::from_utf8(buffer).map_err(|_| FileSystemError::IoError("Invalid UTF-8".into()))
    }

    fn exists(&mut self, path: &str) -> bool {
        self.file_info(path).is_ok()
    }

    fn file_info(&mut self, path: &str) -> Result<FileInfo, FileSystemError> {
        let files = self.list_files("/")?;
        let filename = path.trim_start_matches('/').to_lowercase();
        files
            .into_iter()
            .find(|f| f.name == filename)
            .ok_or(FileSystemError::NotFound)
    }
}
