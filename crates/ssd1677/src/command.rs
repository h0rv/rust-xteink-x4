// SSD1677 command definitions

// Initialization and reset
pub const SOFT_RESET: u8 = 0x12; // Soft reset
pub const BOOSTER_SOFT_START: u8 = 0x0C; // Booster soft-start control
pub const DRIVER_OUTPUT_CONTROL: u8 = 0x01; // Driver output control
pub const BORDER_WAVEFORM: u8 = 0x3C; // Border waveform control
pub const TEMP_SENSOR_CONTROL: u8 = 0x18; // Temperature sensor control

// RAM and buffer management
pub const DATA_ENTRY_MODE: u8 = 0x11; // Data entry mode
pub const SET_RAM_X_RANGE: u8 = 0x44; // Set RAM X address range
pub const SET_RAM_Y_RANGE: u8 = 0x45; // Set RAM Y address range
pub const SET_RAM_X_COUNTER: u8 = 0x4E; // Set RAM X address counter
pub const SET_RAM_Y_COUNTER: u8 = 0x4F; // Set RAM Y address counter
pub const WRITE_RAM_BW: u8 = 0x24; // Write to BW RAM (current frame)
pub const WRITE_RAM_RED: u8 = 0x26; // Write to RED RAM (used for fast refresh)
pub const AUTO_WRITE_BW_RAM: u8 = 0x46; // Auto write BW RAM
pub const AUTO_WRITE_RED_RAM: u8 = 0x47; // Auto write RED RAM

// Display update and refresh
pub const DISPLAY_UPDATE_CTRL1: u8 = 0x21; // Display update control 1
pub const DISPLAY_UPDATE_CTRL2: u8 = 0x22; // Display update control 2
pub const MASTER_ACTIVATION: u8 = 0x20; // Master activation
pub const CTRL1_NORMAL: u8 = 0x00; // Normal mode - compare RED vs BW for partial
pub const CTRL1_BYPASS_RED: u8 = 0x40; // Bypass RED RAM (treat as 0) - for full refresh

// LUT and voltage settings
pub const WRITE_LUT: u8 = 0x32; // Write LUT
pub const GATE_VOLTAGE: u8 = 0x03; // Gate voltage
pub const SOURCE_VOLTAGE: u8 = 0x04; // Source voltage
pub const WRITE_VCOM: u8 = 0x2C; // Write VCOM
pub const WRITE_TEMP: u8 = 0x1A; // Write temperature

// Power management
pub const DEEP_SLEEP: u8 = 0x10; // Deep sleep
