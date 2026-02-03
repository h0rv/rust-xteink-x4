use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ptr;
use std::fs;
use std::io::Write;
use std::path::Path;

use esp_idf_svc::hal::gpio::Pin;
use esp_idf_svc::hal::spi::SpiDriver;
use esp_idf_svc::sys;
use xteink_ui::filesystem::{FileInfo, FileSystem, FileSystemError};

const SD_MOUNT_POINT: &str = "/sd";
const SD_MAX_FILES: i32 = 16;

pub struct SdCardFs {
    base_path: String,
}

impl SdCardFs {
    pub fn new(spi: &SpiDriver, cs_pin: impl Pin) -> Result<Self, FileSystemError> {
        let base_path = SD_MOUNT_POINT.to_string();
        let c_base = std::ffi::CString::new(base_path.clone())
            .map_err(|_| FileSystemError::IoError("Invalid mount path".into()))?;

        let host = build_sdspi_host(spi.host());
        let slot_config = sys::sdspi_device_config_t {
            host_id: spi.host(),
            gpio_cs: cs_pin.pin(),
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

        let res = unsafe {
            sys::esp_vfs_fat_sdspi_mount(
                c_base.as_ptr(),
                &host,
                &slot_config,
                &mount_config,
                ptr::null_mut(),
            )
        };

        if res != sys::ESP_OK {
            return Err(FileSystemError::IoError(format!(
                "SD mount failed: {}",
                res
            )));
        }

        log::info!("SD card mounted at {}", base_path);

        Ok(Self { base_path })
    }

    fn host_path(&self, path: &str) -> String {
        if path == "/" {
            self.base_path.clone()
        } else {
            format!("{}/{}", self.base_path, path.trim_start_matches('/'))
        }
    }

    pub fn delete_file(&mut self, path: &str) -> Result<(), FileSystemError> {
        let host_path = self.host_path(path);
        fatfs_remove_entry(&self.base_path, &host_path)
            .or_else(|_| fs::remove_file(host_path).map_err(to_fs_error))
    }

    pub fn delete_dir(&mut self, path: &str) -> Result<(), FileSystemError> {
        let host_path = self.host_path(path);
        remove_dir_recursive(&self.base_path, &host_path)
    }

    pub fn make_dir(&mut self, path: &str) -> Result<(), FileSystemError> {
        fs::create_dir(self.host_path(path))
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
        let host_path = self.host_path(path);
        let mut file = fs::File::create(host_path)
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
            file.write_all(&buffer[..read])
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
            remaining = remaining.saturating_sub(read);
            written += read;
            on_progress(written)?;
        }

        Ok(())
    }
}

fn remove_dir_recursive(base_path: &str, path: &str) -> Result<(), FileSystemError> {
    for entry in fs::read_dir(path).map_err(to_fs_error)? {
        let entry = entry.map_err(to_fs_error)?;
        let entry_path = entry.path();
        let entry_str = entry_path.to_string_lossy().to_string();
        let meta = entry.metadata().map_err(to_fs_error)?;
        if meta.is_dir() {
            remove_dir_recursive(base_path, &entry_str)?;
        } else {
            let _ = fatfs_remove_entry(base_path, &entry_str);
            let _ = fs::remove_file(&entry_str);
        }
    }

    fatfs_remove_entry(base_path, path).or_else(|_| fs::remove_dir(path).map_err(to_fs_error))
}

fn fatfs_remove_entry(base_path: &str, host_path: &str) -> Result<(), FileSystemError> {
    let fatfs_path = fatfs_path(base_path, host_path)?;
    unsafe {
        sys::f_chmod(fatfs_path.as_ptr(), 0, sys::AM_RDO as u8);
        let res = sys::f_unlink(fatfs_path.as_ptr());
        if res == 0 {
            Ok(())
        } else {
            Err(FileSystemError::IoError(format!("fatfs unlink: {}", res)))
        }
    }
}

fn fatfs_path(base_path: &str, host_path: &str) -> Result<std::ffi::CString, FileSystemError> {
    let rel = host_path
        .strip_prefix(base_path)
        .unwrap_or(host_path)
        .trim_start_matches('/');
    let fat_path = format!("0:/{}", rel);
    std::ffi::CString::new(fat_path)
        .map_err(|_| FileSystemError::IoError("Invalid FATFS path".into()))
}

fn to_fs_error(err: std::io::Error) -> FileSystemError {
    FileSystemError::IoError(format!("{:?}", err))
}

impl FileSystem for SdCardFs {
    fn list_files(&mut self, path: &str) -> Result<Vec<FileInfo>, FileSystemError> {
        let host_path = self.host_path(path);
        let mut entries = Vec::new();

        let read_dir =
            fs::read_dir(&host_path).map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;

        for entry in read_dir {
            let entry = entry.map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
            let meta = entry
                .metadata()
                .map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
            let name = entry.file_name().to_string_lossy().to_string();
            entries.push(FileInfo {
                name,
                size: if meta.is_file() { meta.len() } else { 0 },
                is_directory: meta.is_dir(),
            });
        }

        Ok(entries)
    }

    fn read_file(&mut self, path: &str) -> Result<String, FileSystemError> {
        let host_path = self.host_path(path);
        fs::read_to_string(host_path).map_err(|e| FileSystemError::IoError(format!("{:?}", e)))
    }

    fn exists(&mut self, path: &str) -> bool {
        Path::new(&self.host_path(path)).exists()
    }

    fn file_info(&mut self, path: &str) -> Result<FileInfo, FileSystemError> {
        let host_path = self.host_path(path);
        let meta =
            fs::metadata(&host_path).map_err(|e| FileSystemError::IoError(format!("{:?}", e)))?;
        let name = Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());
        Ok(FileInfo {
            name,
            size: if meta.is_file() { meta.len() } else { 0 },
            is_directory: meta.is_dir(),
        })
    }
}

fn build_sdspi_host(host_id: sys::spi_host_device_t) -> sys::sdmmc_host_t {
    const SDMMC_HOST_FLAG_SPI: u32 = 1 << 3;
    const SDMMC_HOST_FLAG_DEINIT_ARG: u32 = 1 << 5;

    sys::sdmmc_host_t {
        flags: SDMMC_HOST_FLAG_SPI | SDMMC_HOST_FLAG_DEINIT_ARG,
        slot: host_id as _,
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
        dma_aligned_buffer: ptr::null_mut(),
        pwr_ctrl_handle: ptr::null_mut(),
        get_dma_info: Some(sys::sdspi_host_get_dma_info),
    }
}
