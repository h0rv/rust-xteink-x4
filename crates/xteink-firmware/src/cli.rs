extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use esp_idf_svc::sys;

use crate::filesystem::FileSystemError;

pub struct SerialCli {
    buffer: Vec<u8>,
}

impl SerialCli {
    pub fn new() -> Self {
        unsafe {
            let mut config = sys::usb_serial_jtag_driver_config_t {
                tx_buffer_size: 1024,
                rx_buffer_size: 1024,
            };
            sys::usb_serial_jtag_driver_install(&mut config as *mut _);
            sys::esp_vfs_usb_serial_jtag_use_driver();
        }
        Self { buffer: Vec::new() }
    }

    pub fn poll_line(&mut self) -> Option<String> {
        let mut temp = [0u8; 64];
        // Use a short timed read instead of 0-timeout polling to avoid
        // potential busy/lockup behavior in the USB-Serial/JTAG driver.
        let read = self.read_bytes(&mut temp, 1);
        if read <= 0 {
            return None;
        }

        for &b in &temp[..read as usize] {
            match b {
                b'\n' => {
                    let raw = String::from_utf8_lossy(&self.buffer);
                    let cleaned: String = raw
                        .chars()
                        .filter(|ch| ch.is_ascii_graphic() || *ch == ' ')
                        .collect();
                    let line = cleaned.trim().to_string();
                    self.buffer.clear();
                    if line.is_empty() {
                        return None;
                    }
                    return Some(line);
                }
                b'\r' => {}
                _ => {
                    self.buffer.push(b);
                    if self.buffer.len() > 1024 {
                        self.buffer.clear();
                    }
                }
            }
        }

        None
    }

    pub fn write_str(&self, text: &str) {
        unsafe {
            sys::usb_serial_jtag_write_bytes(text.as_ptr().cast(), text.len(), 0);
        }
    }

    pub fn write_line(&self, text: &str) {
        self.write_str(text);
        self.write_str("\r\n");
    }

    pub fn read_bytes(&self, buf: &mut [u8], timeout_ms: u32) -> i32 {
        let ticks = if timeout_ms == 0 {
            0
        } else {
            let rate = sys::configTICK_RATE_HZ as u64;
            let ms = timeout_ms as u64;
            let ticks = (ms * rate + 999) / 1000;
            ticks.max(1) as sys::TickType_t
        };
        unsafe { sys::usb_serial_jtag_read_bytes(buf.as_mut_ptr().cast(), buf.len() as u32, ticks) }
    }

    pub fn read_exact(&self, buf: &mut [u8], timeout_ms: u32) -> Result<(), FileSystemError> {
        let mut offset = 0usize;
        let start_us: i64 = unsafe { sys::esp_timer_get_time() };
        while offset < buf.len() {
            let read = self.read_bytes(&mut buf[offset..], 200);
            if read < 0 {
                return Err(FileSystemError::IoError("UART read failed".into()));
            }
            if read == 0 {
                let now_us: i64 = unsafe { sys::esp_timer_get_time() };
                if (now_us - start_us) > (timeout_ms as i64 * 1000) {
                    return Err(FileSystemError::IoError("UART timeout".into()));
                }
                continue;
            }
            offset += read as usize;
        }
        Ok(())
    }
}
