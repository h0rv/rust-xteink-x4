use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ffi::c_void;
use std::ffi::CString;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

use esp_idf_svc::sys;
use xteink_ui::filesystem::{resolve_mount_path, FileInfo, FileSystem, FileSystemError};

const SD_MOUNT_POINT: &str = "/sd";
const SD_MAX_FILES: i32 = 16;

pub struct SdCardFs {
    mounted: bool,
    mount_error: Option<String>,
    mount_path: CString,
    card_ptr: *mut c_void,
}

impl SdCardFs {
    pub fn new(spi_host: i32, cs_gpio: i32) -> Result<Self, FileSystemError> {
        let mount_path = CString::new(SD_MOUNT_POINT)
            .map_err(|_| FileSystemError::IoError("Invalid mount path".into()))?;

        let mut card_ptr: *mut c_void = core::ptr::null_mut();
        let host = sys::sdmmc_host_t {
            // _SDMMC_HOST_FLAG_SPI (1 << 3) | _SDMMC_HOST_FLAG_DEINIT_ARG (1 << 5)
            flags: (1u32 << 3) | (1u32 << 5),
            slot: spi_host,
            max_freq_khz: 20_000,
            io_voltage: 3.3,
            init: Some(sys::sdspi_host_init),
            set_bus_width: None,
            get_bus_width: None,
            set_bus_ddr_mode: None,
            set_card_clk: Some(sys::sdspi_host_set_card_clk),
            set_cclk_always_on: None,
            do_transaction: Some(sys::sdspi_host_do_transaction),
            __bindgen_anon_1: sys::sdmmc_host_t__bindgen_ty_1 {
                deinit_p: Some(sys::sdspi_host_remove_device),
            },
            io_int_enable: Some(sys::sdspi_host_io_int_enable),
            io_int_wait: Some(sys::sdspi_host_io_int_wait),
            command_timeout_ms: 0,
            get_real_freq: Some(sys::sdspi_host_get_real_freq),
            input_delay_phase: sys::sdmmc_delay_phase_t_SDMMC_DELAY_PHASE_0,
            set_input_delay: None,
            dma_aligned_buffer: core::ptr::null_mut(),
            pwr_ctrl_handle: core::ptr::null_mut(),
            get_dma_info: Some(sys::sdspi_host_get_dma_info),
        };

        let slot_config = sys::sdspi_device_config_t {
            host_id: spi_host,
            gpio_cs: cs_gpio,
            gpio_cd: -1,
            gpio_wp: -1,
            gpio_int: -1,
            gpio_wp_polarity: false,
        };

        let mount_config = sys::esp_vfs_fat_mount_config_t {
            format_if_mount_failed: false,
            max_files: SD_MAX_FILES,
            allocation_unit_size: 0,
            disk_status_check_enable: false,
            use_one_fat: false,
        };

        let err = unsafe {
            sys::esp_vfs_fat_sdspi_mount(
                mount_path.as_ptr(),
                &host,
                &slot_config,
                &mount_config,
                &mut card_ptr as *mut *mut c_void as *mut *mut sys::sdmmc_card_t,
            )
        };

        if err != sys::ESP_OK {
            return Err(FileSystemError::IoError(format!(
                "SD mount failed: {}",
                err
            )));
        }

        Ok(Self {
            mounted: true,
            mount_error: None,
            mount_path,
            card_ptr,
        })
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            mounted: false,
            mount_error: Some(reason.into()),
            mount_path: CString::new(SD_MOUNT_POINT).expect("static mount path must be valid"),
            card_ptr: core::ptr::null_mut(),
        }
    }

    fn ensure_mounted(&self) -> Result<(), FileSystemError> {
        if self.mounted {
            Ok(())
        } else {
            Err(FileSystemError::IoError(format!(
                "SD unavailable: {}",
                self.mount_error.as_deref().unwrap_or("not mounted")
            )))
        }
    }

    fn host_path(&self, path: &str) -> String {
        resolve_mount_path(path, SD_MOUNT_POINT)
    }

    fn split_dir_file(path: &str) -> (&str, &str) {
        match path.rfind('/') {
            Some(0) => ("/", &path[1..]),
            Some(i) => (&path[..i], &path[i + 1..]),
            None => ("/", path),
        }
    }

    pub fn delete_file(&mut self, path: &str) -> Result<(), FileSystemError> {
        self.ensure_mounted()?;
        fs::remove_file(self.host_path(path))
            .map_err(|e| FileSystemError::IoError(format!("remove_file failed: {}", e)))
    }

    pub fn delete_dir(&mut self, path: &str) -> Result<(), FileSystemError> {
        self.ensure_mounted()?;
        fs::remove_dir_all(self.host_path(path))
            .map_err(|e| FileSystemError::IoError(format!("remove_dir_all failed: {}", e)))
    }

    pub fn make_dir(&mut self, path: &str) -> Result<(), FileSystemError> {
        self.ensure_mounted()?;
        fs::create_dir_all(self.host_path(path))
            .map_err(|e| FileSystemError::IoError(format!("create_dir_all failed: {}", e)))
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
        self.ensure_mounted()?;

        let host_path = self.host_path(path);
        let (dir, _) = Self::split_dir_file(&host_path);
        fs::create_dir_all(dir)
            .map_err(|e| FileSystemError::IoError(format!("create parent dir failed: {}", e)))?;

        let mut file = fs::File::create(&host_path)
            .map_err(|e| FileSystemError::IoError(format!("create file failed: {}", e)))?;

        let mut buffer = vec![0u8; chunk_size.max(1)];
        let mut remaining = total_size;
        let mut written = 0usize;

        while remaining > 0 {
            let to_read = remaining.min(buffer.len());
            let read = read_chunk(&mut buffer[..to_read])?;
            if read != to_read {
                return Err(FileSystemError::IoError("Short read".into()));
            }
            file.write_all(&buffer[..read])
                .map_err(|e| FileSystemError::IoError(format!("write failed: {}", e)))?;
            remaining -= read;
            written += read;
            on_progress(written)?;
        }

        Ok(())
    }
}

impl Drop for SdCardFs {
    fn drop(&mut self) {
        if self.mounted && !self.card_ptr.is_null() {
            unsafe {
                let _ = sys::esp_vfs_fat_sdcard_unmount(
                    self.mount_path.as_ptr(),
                    self.card_ptr as *mut sys::sdmmc_card_t,
                );
            }
        }
    }
}

impl FileSystem for SdCardFs {
    fn list_files(&mut self, path: &str) -> Result<Vec<FileInfo>, FileSystemError> {
        self.ensure_mounted()?;
        let mut entries = Vec::new();
        let host_path = self.host_path(path);
        let read_dir = fs::read_dir(&host_path)
            .map_err(|e| FileSystemError::IoError(format!("read_dir failed: {}", e)))?;

        for entry in read_dir {
            let entry =
                entry.map_err(|e| FileSystemError::IoError(format!("dir entry failed: {}", e)))?;
            let meta = entry
                .metadata()
                .map_err(|e| FileSystemError::IoError(format!("metadata failed: {}", e)))?;
            let name = entry.file_name().to_string_lossy().into_owned().to_string();
            entries.push(FileInfo {
                name,
                size: if meta.is_dir() { 0 } else { meta.len() },
                is_directory: meta.is_dir(),
            });
        }

        Ok(entries)
    }

    fn read_file(&mut self, path: &str) -> Result<String, FileSystemError> {
        self.ensure_mounted()?;
        fs::read_to_string(self.host_path(path))
            .map_err(|e| FileSystemError::IoError(format!("read_to_string failed: {}", e)))
    }

    fn read_file_bytes(&mut self, path: &str) -> Result<Vec<u8>, FileSystemError> {
        self.ensure_mounted()?;
        fs::read(self.host_path(path))
            .map_err(|e| FileSystemError::IoError(format!("read failed: {}", e)))
    }

    fn read_file_chunks(
        &mut self,
        path: &str,
        chunk_size: usize,
        on_chunk: &mut dyn FnMut(&[u8]) -> Result<(), FileSystemError>,
    ) -> Result<(), FileSystemError> {
        self.ensure_mounted()?;
        let mut file = fs::File::open(self.host_path(path))
            .map_err(|e| FileSystemError::IoError(format!("open failed: {}", e)))?;
        let mut chunk = vec![0u8; chunk_size.max(1)];
        loop {
            let read = file
                .read(&mut chunk)
                .map_err(|e| FileSystemError::IoError(format!("read failed: {}", e)))?;
            if read == 0 {
                break;
            }
            on_chunk(&chunk[..read])?;
        }
        Ok(())
    }

    fn exists(&mut self, path: &str) -> bool {
        self.ensure_mounted().is_ok() && PathBuf::from(self.host_path(path)).exists()
    }

    fn file_info(&mut self, path: &str) -> Result<FileInfo, FileSystemError> {
        self.ensure_mounted()?;
        let host_path = self.host_path(path);
        let meta = fs::metadata(&host_path)
            .map_err(|e| FileSystemError::IoError(format!("metadata failed: {}", e)))?;
        let name = if path == "/" {
            "/".to_string()
        } else {
            host_path.rsplit('/').next().unwrap_or("").to_string()
        };
        Ok(FileInfo {
            name,
            size: if meta.is_dir() { 0 } else { meta.len() },
            is_directory: meta.is_dir(),
        })
    }
}
