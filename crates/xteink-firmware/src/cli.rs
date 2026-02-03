extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use esp_idf_svc::sys;

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
        let read = unsafe {
            sys::usb_serial_jtag_read_bytes(temp.as_mut_ptr().cast(), temp.len() as u32, 0)
        };
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
}
