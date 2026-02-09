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
    volume_mgr: Option<VolumeManager<SdCard<SPI, DummyCsPin, FreeRtos>, DummyTimeSource, 4, 4, 1>>,
    mount_error: Option<String>,
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
        let size_bytes = sdcard
            .num_bytes()
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        log::info!("SD card size: {} bytes", size_bytes);

        let mut volume_mgr = VolumeManager::new(sdcard, DummyTimeSource);
        let _ = volume_mgr
            .open_volume(VolumeIdx(0))
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

        Ok(Self {
            volume_mgr: Some(volume_mgr),
            mount_error: None,
        })
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            volume_mgr: None,
            mount_error: Some(reason.into()),
        }
    }

    fn volume_mgr(
        &mut self,
    ) -> Result<
        &mut VolumeManager<SdCard<SPI, DummyCsPin, FreeRtos>, DummyTimeSource, 4, 4, 1>,
        FileSystemError,
    > {
        self.volume_mgr.as_mut().ok_or_else(|| {
            FileSystemError::IoError(format!(
                "SD unavailable: {}",
                self.mount_error.as_deref().unwrap_or("not mounted")
            ))
        })
    }

    fn walk_to_dir(
        dir: &mut embedded_sdmmc::Directory<
            SdCard<SPI, DummyCsPin, FreeRtos>,
            DummyTimeSource,
            4,
            4,
            1,
        >,
        path: &str,
    ) -> Result<(), FileSystemError> {
        let clean = path.trim_matches('/');
        for part in clean.split('/').filter(|part| !part.is_empty()) {
            dir.change_dir(part)
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        }
        Ok(())
    }

    pub fn delete_file(&mut self, path: &str) -> Result<(), FileSystemError> {
        let (dir_path, filename) = Self::split_dir_file(path);
        if filename.is_empty() {
            return Err(FileSystemError::IoError("Invalid path".into()));
        }

        let volume_mgr = self.volume_mgr()?;
        let mut volume = volume_mgr
            .open_volume(VolumeIdx(0))
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        let mut dir = volume
            .open_root_dir()
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        Self::walk_to_dir(&mut dir, dir_path)?;
        dir.delete_file_in_dir(filename)
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))
    }

    pub fn delete_dir(&mut self, _path: &str) -> Result<(), FileSystemError> {
        Err(FileSystemError::IoError(
            "Directory deletion unsupported by embedded-sdmmc backend".into(),
        ))
    }

    pub fn make_dir(&mut self, path: &str) -> Result<(), FileSystemError> {
        let (dir_path, name) = Self::split_dir_file(path);
        if name.is_empty() {
            return Err(FileSystemError::IoError("Invalid path".into()));
        }

        let volume_mgr = self.volume_mgr()?;
        let mut volume = volume_mgr
            .open_volume(VolumeIdx(0))
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        let mut dir = volume
            .open_root_dir()
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        Self::walk_to_dir(&mut dir, dir_path)?;
        dir.make_dir_in_dir(name)
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))
    }

    pub fn write_file_streamed<F, G>(
        &mut self,
        path: &str,
        total_size: usize,
        chunk_size: usize,
        mut read_chunk: F,
        mut on_progress: G,
    ) -> Result<(), FileSystemError>
    where
        F: FnMut(&mut [u8]) -> Result<usize, FileSystemError>,
        G: FnMut(usize) -> Result<(), FileSystemError>,
    {
        let (dir_path, filename) = Self::split_dir_file(path);
        if filename.is_empty() {
            return Err(FileSystemError::IoError("Invalid path".into()));
        }

        let volume_mgr = self.volume_mgr()?;
        let mut volume = volume_mgr
            .open_volume(VolumeIdx(0))
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        let mut dir = volume
            .open_root_dir()
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        Self::walk_to_dir(&mut dir, dir_path)?;
        let mut file = dir
            .open_file_in_dir(filename, Mode::ReadWriteCreateOrTruncate)
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

        let mut buffer = vec![0u8; chunk_size.max(1)];
        let mut remaining = total_size;
        let mut written = 0usize;
        while remaining > 0 {
            let to_read = remaining.min(buffer.len());
            let read = read_chunk(&mut buffer[..to_read])?;
            if read != to_read {
                return Err(FileSystemError::IoError("Short read".into()));
            }
            file.write(&buffer[..read])
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
            remaining = remaining.saturating_sub(read);
            written += read;
            on_progress(written)?;
        }

        Ok(())
    }
}

impl<SPI> FileSystem for SdCardFs<SPI>
where
    SPI: embedded_hal::spi::SpiDevice,
{
    fn list_files(&mut self, path: &str) -> Result<Vec<FileInfo>, FileSystemError> {
        let volume_mgr = self.volume_mgr()?;
        let mut volume = volume_mgr
            .open_volume(VolumeIdx(0))
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        let mut dir = volume
            .open_root_dir()
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        Self::walk_to_dir(&mut dir, path)?;

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

            if !full_name.is_empty() {
                files.push(FileInfo {
                    name: full_name,
                    size: if entry.attributes.is_directory() {
                        0
                    } else {
                        entry.size as u64
                    },
                    is_directory: entry.attributes.is_directory(),
                });
            }
        })
        .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

        Ok(files)
    }

    fn read_file(&mut self, path: &str) -> Result<String, FileSystemError> {
        let bytes = self.read_file_bytes(path)?;
        String::from_utf8(bytes).map_err(|_| FileSystemError::IoError("Invalid UTF-8".into()))
    }

    fn read_file_bytes(&mut self, path: &str) -> Result<Vec<u8>, FileSystemError> {
        let (dir_path, filename) = Self::split_dir_file(path);
        if filename.is_empty() {
            return Err(FileSystemError::IoError("Invalid path".into()));
        }

        let volume_mgr = self.volume_mgr()?;
        let mut volume = volume_mgr
            .open_volume(VolumeIdx(0))
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        let mut dir = volume
            .open_root_dir()
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        Self::walk_to_dir(&mut dir, dir_path)?;
        let mut file = dir
            .open_file_in_dir(filename, Mode::ReadOnly)
            .map_err(|_| FileSystemError::NotFound)?;

        let file_size = file.length() as usize;
        let mut buffer = Vec::with_capacity(file_size);
        let mut chunk = [0u8; 512];

        loop {
            let read = file
                .read(&mut chunk)
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
        }

        Ok(buffer)
    }

    fn read_file_chunks(
        &mut self,
        path: &str,
        chunk_size: usize,
        on_chunk: &mut dyn FnMut(&[u8]) -> Result<(), FileSystemError>,
    ) -> Result<(), FileSystemError> {
        let (dir_path, filename) = Self::split_dir_file(path);
        if filename.is_empty() {
            return Err(FileSystemError::IoError("Invalid path".into()));
        }

        let volume_mgr = self.volume_mgr()?;
        let mut volume = volume_mgr
            .open_volume(VolumeIdx(0))
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        let mut dir = volume
            .open_root_dir()
            .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        Self::walk_to_dir(&mut dir, dir_path)?;
        let mut file = dir
            .open_file_in_dir(filename, Mode::ReadOnly)
            .map_err(|_| FileSystemError::NotFound)?;

        let mut chunk = vec![0u8; chunk_size.max(1)];
        loop {
            let read = file
                .read(&mut chunk)
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
            if read == 0 {
                break;
            }
            on_chunk(&chunk[..read])?;
        }
        Ok(())
    }

    fn exists(&mut self, path: &str) -> bool {
        self.file_info(path).is_ok()
    }

    fn file_info(&mut self, path: &str) -> Result<FileInfo, FileSystemError> {
        if path == "/" {
            return Ok(FileInfo {
                name: "/".to_string(),
                size: 0,
                is_directory: true,
            });
        }

        let (dir_path, filename) = Self::split_dir_file(path);
        let entries = self.list_files(dir_path)?;
        entries
            .into_iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(filename))
            .ok_or(FileSystemError::NotFound)
    }
}
