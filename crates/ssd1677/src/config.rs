//! Display configuration types and builder

pub use crate::error::{BuilderError, MAX_GATE_OUTPUTS, MAX_SOURCE_OUTPUTS};

/// Display dimensions
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dimensions {
    /// Number of rows (height in pixels, corresponds to gate outputs)
    pub rows: u16,
    /// Number of columns (width in pixels, corresponds to source outputs)
    pub cols: u16,
}

impl Dimensions {
    /// Create new dimensions with validation
    ///
    /// # Errors
    ///
    /// Returns `BuilderError::InvalidDimensions` if:
    /// - rows > MAX_GATE_OUTPUTS
    /// - cols > MAX_SOURCE_OUTPUTS
    /// - cols % 8 != 0 (must be byte-aligned for memory)
    pub fn new(rows: u16, cols: u16) -> Result<Self, BuilderError> {
        if rows == 0 || rows > MAX_GATE_OUTPUTS {
            return Err(BuilderError::InvalidDimensions { rows, cols });
        }
        if cols == 0 || cols > MAX_SOURCE_OUTPUTS || !cols.is_multiple_of(8) {
            return Err(BuilderError::InvalidDimensions { rows, cols });
        }
        Ok(Self { rows, cols })
    }

    /// Calculate required buffer size in bytes
    pub fn buffer_size(&self) -> usize {
        (self.rows as usize * self.cols as usize) / 8
    }
}

/// Display rotation relative to native orientation
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Rotation {
    /// No rotation
    #[default]
    Rotate0,
    /// Rotate 90 degrees clockwise
    Rotate90,
    /// Rotate 180 degrees
    Rotate180,
    /// Rotate 270 degrees clockwise
    Rotate270,
}

/// Display configuration
///
/// This struct holds all configurable parameters for the SSD1677 controller.
/// Use `Builder` to create a Config.
#[derive(Clone, Debug)]
pub struct Config {
    /// Display dimensions
    pub dimensions: Dimensions,
    /// Display rotation
    pub rotation: Rotation,
    /// Booster soft-start settings (5 bytes for command 0x0C)
    pub booster_soft_start: [u8; 5],
    /// Gate scanning direction byte
    pub gate_scanning: u8,
    /// Border waveform setting
    pub border_waveform: u8,
    /// VCOM register value
    pub vcom: u8,
    /// Data entry mode byte
    pub data_entry_mode: u8,
    /// Temperature sensor control
    pub temp_sensor_control: u8,
}

impl Config {
    /// Get the rotated dimensions based on rotation setting
    pub fn rotated_dimensions(&self) -> Dimensions {
        match self.rotation {
            Rotation::Rotate0 | Rotation::Rotate180 => self.dimensions,
            Rotation::Rotate90 | Rotation::Rotate270 => Dimensions {
                rows: self.dimensions.cols,
                cols: self.dimensions.rows,
            },
        }
    }
}

/// Builder for constructing display configuration
///
/// # Example
///
/// ```
/// use ssd1677::{Builder, Dimensions, Rotation};
///
/// let config = Builder::new()
///     .dimensions(Dimensions::new(480, 480).unwrap())
///     .rotation(Rotation::Rotate0)
///     .build()
///     .expect("valid configuration");
/// ```
pub struct Builder {
    /// Display dimensions (required)
    dimensions: Option<Dimensions>,
    /// Display rotation
    rotation: Rotation,
    /// Booster soft-start settings (5 bytes for command 0x0C)
    booster_soft_start: [u8; 5],
    /// Gate scanning direction byte
    gate_scanning: u8,
    /// Border waveform setting
    border_waveform: u8,
    /// VCOM register value
    vcom: u8,
    /// Data entry mode byte
    data_entry_mode: u8,
    /// Temperature sensor control
    temp_sensor_control: u8,
}

impl Default for Builder {
    fn default() -> Self {
        Builder {
            dimensions: None,
            rotation: Rotation::Rotate0,
            // Default booster soft-start sequence (0xAE, 0xC7, 0xC3, 0xC0, 0x40)
            booster_soft_start: [0xAE, 0xC7, 0xC3, 0xC0, 0x40],
            // Default gate scanning (from datasheet)
            gate_scanning: 0x02,
            // Default border waveform
            border_waveform: 0x01,
            // Default VCOM
            vcom: 0x3C,
            // Default: X increment, Y decrement (for reversed gates)
            data_entry_mode: 0x01,
            // Default: internal temperature sensor
            temp_sensor_control: 0x80,
        }
    }
}

impl Builder {
    /// Create a new Builder with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set display dimensions (required)
    pub fn dimensions(mut self, dims: Dimensions) -> Self {
        self.dimensions = Some(dims);
        self
    }

    /// Set display rotation
    pub fn rotation(mut self, rotation: Rotation) -> Self {
        self.rotation = rotation;
        self
    }

    /// Set booster soft-start parameters
    pub fn booster_soft_start(mut self, values: [u8; 5]) -> Self {
        self.booster_soft_start = values;
        self
    }

    /// Set gate scanning direction
    pub fn gate_scanning(mut self, value: u8) -> Self {
        self.gate_scanning = value;
        self
    }

    /// Set border waveform
    pub fn border_waveform(mut self, value: u8) -> Self {
        self.border_waveform = value;
        self
    }

    /// Set VCOM value
    pub fn vcom(mut self, value: u8) -> Self {
        self.vcom = value;
        self
    }

    /// Set data entry mode
    pub fn data_entry_mode(mut self, value: u8) -> Self {
        self.data_entry_mode = value;
        self
    }

    /// Set temperature sensor control
    pub fn temp_sensor_control(mut self, value: u8) -> Self {
        self.temp_sensor_control = value;
        self
    }

    /// Build the configuration
    ///
    /// # Errors
    ///
    /// Returns `BuilderError::MissingDimensions` if dimensions were not set
    pub fn build(self) -> Result<Config, BuilderError> {
        Ok(Config {
            dimensions: self.dimensions.ok_or(BuilderError::MissingDimensions)?,
            rotation: self.rotation,
            booster_soft_start: self.booster_soft_start,
            gate_scanning: self.gate_scanning,
            border_waveform: self.border_waveform,
            vcom: self.vcom,
            data_entry_mode: self.data_entry_mode,
            temp_sensor_control: self.temp_sensor_control,
        })
    }
}
