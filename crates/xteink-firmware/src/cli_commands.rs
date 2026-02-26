use einked::input::Button;
use ssd1677::{Display as EinkDisplay, DisplayInterface, RefreshMode};

use crate::buffered_display::BufferedDisplay;
use crate::cli::SerialCli;
use crate::filesystem::{FileSystem, FileSystemError};
use crate::sdcard::SdCardFs;
use crate::wifi_manager::{WifiManager, WifiMode};

fn format_size(size: u64) -> String {
    if size >= 1024 * 1024 {
        format!("{:.1}MB", size as f32 / (1024.0 * 1024.0))
    } else if size >= 1024 {
        format!("{:.0}KB", size as f32 / 1024.0)
    } else {
        format!("{}B", size)
    }
}

fn cli_redraw<I, D>(
    display: &mut EinkDisplay<I>,
    delay: &mut D,
    buffered_display: &mut BufferedDisplay,
    mode: RefreshMode,
) where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    display
        .update_with_mode_no_lut(buffered_display.buffer(), &[], mode, delay)
        .ok();
}

pub fn handle_cli_command<I, D>(
    line: &str,
    cli: &SerialCli,
    fs: &mut impl FsCliOps,
    display: &mut EinkDisplay<I>,
    delay: &mut D,
    buffered_display: &mut BufferedDisplay,
    sleep_requested: &mut bool,
    wifi_manager: &mut WifiManager,
    injected_button: &mut Option<Button>,
) where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    let mut parts = line.split_whitespace();
    let cmd = parts.next().unwrap_or("");

    match cmd {
        "help" => {
            cli.write_line(
                "Commands: help, ls [path], exists <path>, stat <path>, rm <path>, rmdir <path>, mkdir/md <path>, cat <path>",
            );
            cli.write_line(
                "          put <path> <size> [chunk], refresh <full|partial|fast>, sleep",
            );
            cli.write_line(
                "          wifi status|show|mode <ap|sta>|ap <ssid> [pass]|sta <ssid> <pass>|clear",
            );
            cli.write_line("          btn <confirm|back|left|right|aux1|aux2|aux3>");
            cli.write_line("OK");
        }
        "ls" => {
            let path = parts.next().unwrap_or("/");
            match fs.list_files(path) {
                Ok(files) => {
                    for file in files {
                        let kind = if file.is_directory { "D" } else { "F" };
                        let name = if file.is_directory {
                            format!("{}/", file.name)
                        } else {
                            file.name
                        };
                        cli.write_line(&format!("{} {} {}", kind, name, format_size(file.size)));
                    }
                    cli.write_line("OK");
                }
                Err(err) => cli.write_line(&format!("ERR {:?}", err)),
            }
        }
        "exists" => {
            let path = parts.next().unwrap_or("/");
            let exists = fs.exists(path);
            cli.write_line(if exists { "1" } else { "0" });
            cli.write_line("OK");
        }
        "stat" => {
            let path = match parts.next() {
                Some(path) => path,
                None => {
                    cli.write_line("ERR missing path");
                    return;
                }
            };
            match fs.file_info(path) {
                Ok(info) => {
                    let kind = if info.is_directory { "dir" } else { "file" };
                    cli.write_line(&format!("{} {}", kind, info.size));
                    cli.write_line("OK");
                }
                Err(err) => cli.write_line(&format!("ERR {:?}", err)),
            }
        }
        "rm" => {
            let path = match parts.next() {
                Some(path) => path,
                None => {
                    cli.write_line("ERR missing path");
                    return;
                }
            };
            match fs.file_info(path) {
                Ok(info) => {
                    if info.is_directory {
                        cli.write_line("ERR use rmdir for directories");
                        return;
                    }
                }
                Err(err) => {
                    cli.write_line(&format!("ERR {:?}", err));
                    return;
                }
            }
            match fs.delete_file(path) {
                Ok(()) => {
                    cli.write_line("OK");
                }
                Err(err) => cli.write_line(&format!("ERR {:?}", err)),
            }
        }
        "rmdir" => {
            let path = match parts.next() {
                Some(path) => path,
                None => {
                    cli.write_line("ERR missing path");
                    return;
                }
            };
            match fs.file_info(path) {
                Ok(info) => {
                    if !info.is_directory {
                        cli.write_line("ERR not a directory");
                        return;
                    }
                }
                Err(err) => {
                    cli.write_line(&format!("ERR {:?}", err));
                    return;
                }
            }
            match fs.delete_dir(path) {
                Ok(()) => {
                    cli.write_line("OK");
                }
                Err(err) => cli.write_line(&format!("ERR {:?}", err)),
            }
        }
        "mkdir" | "md" => {
            let path = match parts.next() {
                Some(path) => path,
                None => {
                    cli.write_line("ERR missing path");
                    return;
                }
            };
            match fs.make_dir(path) {
                Ok(()) => {
                    cli.write_line("OK");
                }
                Err(err) => cli.write_line(&format!("ERR {:?}", err)),
            }
        }
        "cat" => {
            let path = match parts.next() {
                Some(path) => path,
                None => {
                    cli.write_line("ERR missing path");
                    return;
                }
            };
            match fs.file_info(path) {
                Ok(info) => {
                    if info.size > 16 * 1024 {
                        cli.write_line("ERR file too large");
                        return;
                    }
                }
                Err(err) => {
                    cli.write_line(&format!("ERR {:?}", err));
                    return;
                }
            }
            match fs.read_file(path) {
                Ok(content) => {
                    cli.write_str(&content);
                    cli.write_line("");
                    cli.write_line("OK");
                }
                Err(err) => cli.write_line(&format!("ERR {:?}", err)),
            }
        }
        "put" => {
            let path = match parts.next() {
                Some(path) => path,
                None => {
                    cli.write_line("ERR missing path");
                    return;
                }
            };
            let size: usize = match parts.next().and_then(|value| value.parse().ok()) {
                Some(size) => size,
                None => {
                    cli.write_line("ERR missing size");
                    return;
                }
            };
            let chunk_size: usize = parts
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(1024);

            cli.write_line(&format!("OK READY {}", chunk_size));
            let mut hasher = crc32fast::Hasher::new();
            let res = fs.write_file_streamed(
                path,
                size,
                chunk_size,
                |buf| {
                    cli.read_exact(buf, 5000)?;
                    hasher.update(buf);
                    Ok(buf.len())
                },
                |written| {
                    cli.write_line(&format!("OK {}", written));
                    Ok(())
                },
            );

            if let Err(err) = res {
                cli.write_line(&format!("ERR {:?}", err));
                return;
            }

            let crc = hasher.finalize();
            cli.write_line(&format!("OK DONE {:08x}", crc));
        }
        "refresh" => {
            let mode = match parts.next().unwrap_or("fast") {
                "full" => RefreshMode::Full,
                "partial" => RefreshMode::Partial,
                _ => RefreshMode::Fast,
            };
            cli_redraw(display, delay, buffered_display, mode);
            cli.write_line("OK");
        }
        "sleep" => {
            cli.write_line("OK sleeping");
            *sleep_requested = true;
        }
        "btn" => {
            let Some(name) = parts.next() else {
                cli.write_line("ERR missing button");
                return;
            };
            let parsed = match name {
                "confirm" => Some(Button::Confirm),
                "back" => Some(Button::Back),
                "left" => Some(Button::Left),
                "right" => Some(Button::Right),
                "aux1" => Some(Button::Aux1),
                "aux2" => Some(Button::Aux2),
                "aux3" => Some(Button::Aux3),
                _ => None,
            };
            let Some(btn) = parsed else {
                cli.write_line("ERR button must be confirm|back|left|right|aux1|aux2|aux3");
                return;
            };
            *injected_button = Some(btn);
            cli.write_line("OK");
        }
        "wifi" => {
            let sub = parts.next().unwrap_or("status");
            match sub {
                "status" | "show" => {
                    let settings = wifi_manager.settings();
                    cli.write_line(&format!("mode {}", settings.mode.as_str()));
                    cli.write_line(&format!(
                        "active {}",
                        if wifi_manager.is_network_active() {
                            1
                        } else {
                            0
                        }
                    ));
                    cli.write_line(&format!("ap_ssid {}", settings.ap_ssid));
                    cli.write_line(&format!(
                        "ap_password {}",
                        WifiManager::masked_password(&settings.ap_password)
                    ));
                    cli.write_line(&format!("sta_ssid {}", settings.sta_ssid));
                    cli.write_line(&format!(
                        "sta_password {}",
                        WifiManager::masked_password(&settings.sta_password)
                    ));
                    let info = wifi_manager.transfer_info();
                    if !info.url.is_empty() {
                        cli.write_line(&format!("url {}", info.url));
                    }
                    if !info.message.is_empty() {
                        cli.write_line(&format!("message {}", info.message));
                    }
                    cli.write_line("OK");
                }
                "mode" => {
                    let Some(mode_raw) = parts.next() else {
                        cli.write_line("ERR missing mode");
                        return;
                    };
                    let Some(mode) = WifiMode::from_str(mode_raw) else {
                        cli.write_line("ERR mode must be ap|sta");
                        return;
                    };
                    match wifi_manager.set_mode(mode) {
                        Ok(()) => cli.write_line("OK"),
                        Err(err) => cli.write_line(&format!("ERR {}", err)),
                    }
                }
                "ap" => {
                    let Some(ssid) = parts.next() else {
                        cli.write_line("ERR missing ssid");
                        return;
                    };
                    let password = parts.next().unwrap_or("").to_string();
                    match wifi_manager.configure_ap(ssid.to_string(), password) {
                        Ok(()) => cli.write_line("OK"),
                        Err(err) => cli.write_line(&format!("ERR {}", err)),
                    }
                }
                "sta" => {
                    let Some(ssid) = parts.next() else {
                        cli.write_line("ERR missing ssid");
                        return;
                    };
                    let Some(password) = parts.next() else {
                        cli.write_line("ERR missing password");
                        return;
                    };
                    match wifi_manager.configure_sta(ssid.to_string(), password.to_string()) {
                        Ok(()) => cli.write_line("OK"),
                        Err(err) => cli.write_line(&format!("ERR {}", err)),
                    }
                }
                "clear" => match wifi_manager.clear_sta() {
                    Ok(()) => cli.write_line("OK"),
                    Err(err) => cli.write_line(&format!("ERR {}", err)),
                },
                _ => cli.write_line("ERR unknown wifi command"),
            }
        }
        "" => {}
        _ => cli.write_line("ERR unknown command"),
    }
}

pub trait FsCliOps: FileSystem {
    fn delete_file(&mut self, path: &str) -> Result<(), FileSystemError>;
    fn delete_dir(&mut self, path: &str) -> Result<(), FileSystemError>;
    fn make_dir(&mut self, path: &str) -> Result<(), FileSystemError>;
    fn write_file_streamed<F, G>(
        &mut self,
        path: &str,
        total_size: usize,
        chunk_size: usize,
        read_chunk: F,
        on_progress: G,
    ) -> Result<(), FileSystemError>
    where
        F: FnMut(&mut [u8]) -> Result<usize, FileSystemError>,
        G: FnMut(usize) -> Result<(), FileSystemError>;
}

impl FsCliOps for SdCardFs {
    fn delete_file(&mut self, path: &str) -> Result<(), FileSystemError> {
        SdCardFs::delete_file(self, path)
    }

    fn delete_dir(&mut self, path: &str) -> Result<(), FileSystemError> {
        SdCardFs::delete_dir(self, path)
    }

    fn make_dir(&mut self, path: &str) -> Result<(), FileSystemError> {
        SdCardFs::make_dir(self, path)
    }

    fn write_file_streamed<F, G>(
        &mut self,
        path: &str,
        total_size: usize,
        chunk_size: usize,
        read_chunk: F,
        on_progress: G,
    ) -> Result<(), FileSystemError>
    where
        F: FnMut(&mut [u8]) -> Result<usize, FileSystemError>,
        G: FnMut(usize) -> Result<(), FileSystemError>,
    {
        SdCardFs::write_file_streamed(self, path, total_size, chunk_size, read_chunk, on_progress)
    }
}
