//! SD card filesystem using embedded-sdmmc

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use embedded_sdmmc::{
    sdcard::DummyCsPin, Mode, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager,
};
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
    fn split_dir_file(path: &str) -> (&str, &str) {
        match path.rfind('/') {
            Some(0) => ("/", &path[1..]),
            Some(i) => (&path[..i], &path[i + 1..]),
            None => ("/", path),
        }
    }

    pub fn new(spi: SPI) -> Result<Self, FileSystemError> {
        let sdcard = SdCard::new(spi, DummyCsPin, FreeRtos);

        log::info!("SD card size: {} bytes", sdcard.num_bytes().unwrap_or(0));

        let volume_mgr = VolumeManager::new(sdcard, DummyTimeSource);
        Ok(Self { volume_mgr })
    }

    pub fn delete_file(&mut self, path: &str) -> Result<(), FileSystemError> {
        let mut volume = self
            .volume_mgr
            .open_volume(VolumeIdx(0))
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

        let (dir_path, filename) = Self::split_dir_file(path);
        if filename.is_empty() {
            return Err(FileSystemError::IoError("Invalid path".into()));
        }

        let mut dir = volume
            .open_root_dir()
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        let clean = dir_path.trim_matches('/');
        for part in clean.split('/').filter(|part| !part.is_empty()) {
            dir.change_dir(part)
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        }
        dir.delete_file_in_dir(filename)
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        Ok(())
    }

    pub fn make_dir(&mut self, path: &str) -> Result<(), FileSystemError> {
        let mut volume = self
            .volume_mgr
            .open_volume(VolumeIdx(0))
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

        let (dir_path, name) = Self::split_dir_file(path);
        if name.is_empty() {
            return Err(FileSystemError::IoError("Invalid path".into()));
        }

        let mut dir = volume
            .open_root_dir()
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        let clean = dir_path.trim_matches('/');
        for part in clean.split('/').filter(|part| !part.is_empty()) {
            dir.change_dir(part)
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        }
        dir.make_dir_in_dir(name)
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        Ok(())
    }

    pub fn write_file_chunks<F>(
        &mut self,
        path: &str,
        total_size: usize,
        mut read_chunk: F,
    ) -> Result<(), FileSystemError>
    where
        F: FnMut(&mut [u8]) -> Result<usize, FileSystemError>,
    {
        let mut volume = self
            .volume_mgr
            .open_volume(VolumeIdx(0))
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

        let (dir_path, filename) = Self::split_dir_file(path);
        if filename.is_empty() {
            return Err(FileSystemError::IoError("Invalid path".into()));
        }

        let mut dir = volume
            .open_root_dir()
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        let clean = dir_path.trim_matches('/');
        for part in clean.split('/').filter(|part| !part.is_empty()) {
            dir.change_dir(part)
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        }
        let mut file = dir
            .open_file_in_dir(filename, Mode::ReadWriteCreateOrTruncate)
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

        let mut chunk = [0u8; 512];
        let mut remaining = total_size;
        while remaining > 0 {
            let to_read = remaining.min(chunk.len());
            let read = read_chunk(&mut chunk[..to_read])?;
            if read == 0 {
                continue;
            }
            if read > to_read {
                return Err(FileSystemError::IoError("Read overflow".into()));
            }
            file.write(&chunk[..read])
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
            remaining = remaining.saturating_sub(read);
        }

        Ok(())
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

        let mut dir = volume
            .open_root_dir()
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        let clean = path.trim_matches('/');
        for part in clean.split('/').filter(|part| !part.is_empty()) {
            dir.change_dir(part)
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        }

        let mut files = Vec::new();

        dir.iterate_dir(|entry| {
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
                    name: full_name,
                    size: entry.size as u64,
                    is_directory: entry.attributes.is_directory(),
                });
            }
        })
        .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

        Ok(files)
    }

    fn read_file(&mut self, path: &str) -> Result<String, FileSystemError> {
        let mut volume = self
            .volume_mgr
            .open_volume(VolumeIdx(0))
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

        let (dir_path, filename) = Self::split_dir_file(path);
        let mut dir = volume
            .open_root_dir()
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        let clean = dir_path.trim_matches('/');
        for part in clean.split('/').filter(|part| !part.is_empty()) {
            dir.change_dir(part)
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        }
        let mut file = dir
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
        let (dir_path, filename) = Self::split_dir_file(path);
        let files = self.list_files(dir_path)?;
        files
            .into_iter()
            .find(|f| f.name == filename)
            .ok_or(FileSystemError::NotFound)
    }
}
