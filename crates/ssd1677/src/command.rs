//! SSD1677 command definitions
//!
//! This module defines all the command bytes used to control the SSD1677
//! e-paper display controller. Commands are sent over SPI with the DC pin
//! low for commands and high for data.
//!
//! ## Command Structure
//!
//! All commands follow the pattern:
//! 1. Assert CS (Chip Select)
//! 2. Set DC low (command mode)
//! 3. Send command byte
//! 4. Set DC high (data mode)
//! 5. Send data bytes (if any)
//! 6. Deassert CS
//!
//! ## Example
//!
//! ```rust,ignore
//! use ssd1677::command;
//!
//! // Soft reset
//! interface.send_command(command::SOFT_RESET)?;
//!
//! // Write to black/white RAM
//! interface.send_command(command::WRITE_RAM_BW)?;
//! interface.send_data(&pixel_data)?;
//! ```

// System control commands

/// Soft reset command (0x12)
///
/// Resets the controller to default state. Must wait for BUSY low after issuing.
pub const SOFT_RESET: u8 = 0x12;

/// Booster soft-start control command (0x0C)
///
/// Controls the power-on sequence of the booster circuit.
/// Requires 5 bytes of data.
pub const BOOSTER_SOFT_START: u8 = 0x0C;

/// Driver output control command (0x01)
///
/// Sets the number of gate outputs (rows) and scanning direction.
/// Requires 3 bytes: [rows-1 (LSB), rows-1 (MSB), scanning mode]
pub const DRIVER_OUTPUT_CONTROL: u8 = 0x01;

/// Border waveform control command (0x3C)
///
/// Controls the border color and transition behavior.
/// Requires 1 byte of data.
pub const BORDER_WAVEFORM: u8 = 0x3C;

/// Temperature sensor control command (0x18)
///
/// Selects internal or external temperature sensor for optimal refresh timing.
/// Requires 1 byte: 0x80 = internal, 0x48 = external
pub const TEMP_SENSOR_CONTROL: u8 = 0x18;

// RAM and data commands

/// Data entry mode command (0x11)
///
/// Controls the address counter auto-increment direction.
/// Requires 1 byte:
/// - Bit 0: X direction (0=increment, 1=decrement)
/// - Bit 1: Y direction (0=decrement, 1=increment)
/// - Bit 2: Address counter direction (0=X, 1=Y)
pub const DATA_ENTRY_MODE: u8 = 0x11;

/// Set RAM X address range command (0x44)
///
/// Sets the X (column) address range for RAM access.
/// Requires 4 bytes: [start_LSB, start_MSB, end_LSB, end_MSB]
pub const SET_RAM_X_RANGE: u8 = 0x44;

/// Set RAM Y address range command (0x45)
///
/// Sets the Y (row) address range for RAM access.
/// Requires 4 bytes: [start_LSB, start_MSB, end_LSB, end_MSB]
pub const SET_RAM_Y_RANGE: u8 = 0x45;

/// Set RAM X address counter command (0x4E)
///
/// Sets the X address counter to specific value.
/// Requires 2 bytes: [address_LSB, address_MSB]
pub const SET_RAM_X_COUNTER: u8 = 0x4E;

/// Set RAM Y address counter command (0x4F)
///
/// Sets the Y address counter to specific value.
/// Requires 2 bytes: [address_LSB, address_MSB]
pub const SET_RAM_Y_COUNTER: u8 = 0x4F;

/// Write to BW RAM (current frame) command (0x24)
///
/// Writes black/white pixel data to the current frame buffer.
/// Bit=0: Black, Bit=1: White
/// Requires pixel data bytes (width * height / 8).
pub const WRITE_RAM_BW: u8 = 0x24;

/// Write to RED RAM (used for fast refresh) command (0x26)
///
/// Writes red pixel data to the red frame buffer.
/// Bit=1: Red color (overrides BW for that pixel)
/// Requires pixel data bytes (width * height / 8).
pub const WRITE_RAM_RED: u8 = 0x26;

/// Auto write BW RAM command (0x46)
///
/// Automatically fills the entire BW RAM with a single byte value.
/// Requires 1 byte fill value.
pub const AUTO_WRITE_BW_RAM: u8 = 0x46;

/// Auto write RED RAM command (0x47)
///
/// Automatically fills the entire RED RAM with a single byte value.
/// Requires 1 byte fill value.
pub const AUTO_WRITE_RED_RAM: u8 = 0x47;

// Display update commands

/// Display update control 1 command (0x21)
///
/// Controls which RAM sources are used for display update.
/// Requires 1 byte: 0x00 = normal (both), 0x40 = bypass RED
pub const DISPLAY_UPDATE_CTRL1: u8 = 0x21;

/// Display update control 2 command (0x22)
///
/// Controls the display update sequence (power on/off, load LUT, etc).
/// Requires 1 byte with bit flags:
/// - 0x01: Enable clock
/// - 0x02: Enable analog
/// - 0x04: Load temperature value
/// - 0x08: Load LUT
/// - 0x10: Initial display (disable bypass)
/// - 0x20: Pattern display (refresh)
/// - 0x40: Disable analog
/// - 0x80: Disable clock
pub const DISPLAY_UPDATE_CTRL2: u8 = 0x22;

/// Master activation command (0x20)
///
/// Triggers the display update sequence. BUSY goes high during update.
pub const MASTER_ACTIVATION: u8 = 0x20;

/// Normal mode - compare RED vs BW for partial updates
///
/// Used with DISPLAY_UPDATE_CTRL1 to enable both RAM sources.
pub const CTRL1_NORMAL: u8 = 0x00;

/// Bypass RED RAM (treat as 0) - for full refresh
///
/// Used with DISPLAY_UPDATE_CTRL1 to ignore RED RAM for
/// full black/white refresh.
pub const CTRL1_BYPASS_RED: u8 = 0x40;

// Power and LUT commands

/// Write LUT command (0x32)
///
/// Loads a custom Look-Up Table (waveform) for the display update.
/// Requires 112 bytes for SSD1677.
pub const WRITE_LUT: u8 = 0x32;

/// Gate voltage command (0x03)
///
/// Sets the gate driving voltage (VGH).
/// Requires 1 byte.
pub const GATE_VOLTAGE: u8 = 0x03;

/// Source voltage command (0x04)
///
/// Sets the source driving voltages (VSH1, VSH2, VSL).
/// Requires 3 bytes.
pub const SOURCE_VOLTAGE: u8 = 0x04;

/// Write VCOM command (0x2C)
///
/// Sets the VCOM voltage for common electrode.
/// Requires 1 byte.
pub const WRITE_VCOM: u8 = 0x2C;

/// Write temperature command (0x1A)
///
/// Writes temperature value for refresh timing (in 1/16°C units).
/// Requires 2 bytes.
pub const WRITE_TEMP: u8 = 0x1A;

// Power management commands

/// Deep sleep command (0x10)
///
/// Enters ultra-low power mode. Only soft reset can wake.
/// Requires 1 byte: 0x01 = enter deep sleep
pub const DEEP_SLEEP: u8 = 0x10;
