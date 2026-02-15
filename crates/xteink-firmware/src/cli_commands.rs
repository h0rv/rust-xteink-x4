use xteink_ui::filesystem::FileSystemError;
use xteink_ui::{App, BufferedDisplay, DisplayInterface, EinkDisplay, FileSystem, RefreshMode};

use crate::cli::SerialCli;
use crate::sdcard::SdCardFs;

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
    app: &mut App,
    display: &mut EinkDisplay<I>,
    delay: &mut D,
    buffered_display: &mut BufferedDisplay,
    mode: RefreshMode,
) where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    buffered_display.clear();
    app.render(buffered_display).ok();
    display
        .update_with_mode_no_lut(buffered_display.buffer(), &[], mode, delay)
        .ok();
}

pub fn handle_cli_command<I, D>(
    line: &str,
    cli: &SerialCli,
    fs: &mut impl FsCliOps,
    app: &mut App,
    display: &mut EinkDisplay<I>,
    delay: &mut D,
    buffered_display: &mut BufferedDisplay,
    enter_sleep: fn(i32),
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
                    app.invalidate_library_cache();
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
                    app.invalidate_library_cache();
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
                    app.invalidate_library_cache();
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
            app.invalidate_library_cache();
            cli.write_line(&format!("OK DONE {:08x}", crc));
        }
        "refresh" => {
            let mode = match parts.next().unwrap_or("fast") {
                "full" => RefreshMode::Full,
                "partial" => RefreshMode::Partial,
                _ => RefreshMode::Fast,
            };
            cli_redraw(app, display, delay, buffered_display, mode);
            cli.write_line("OK");
        }
        "sleep" => {
            cli.write_line("OK sleeping");
            enter_sleep(3);
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
