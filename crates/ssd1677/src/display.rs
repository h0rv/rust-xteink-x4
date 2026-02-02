//! Core display operations

use embedded_hal::delay::DelayNs;

use crate::command::*;
use crate::config::Config;
use crate::error::Error;
use crate::interface::DisplayInterface;

/// Core display driver for SSD1677
///
/// This struct provides low-level operations for the SSD1677 controller.
/// For graphics support, use `GraphicDisplay` (requires `graphics` feature).
pub struct Display<I>
where
    I: DisplayInterface,
{
    /// Hardware interface
    interface: I,
    /// Display configuration
    config: Config,
    /// Whether the display power is on
    is_display_on: bool,
}

impl<I> Display<I>
where
    I: DisplayInterface,
{
    /// Create a new Display instance
    pub fn new(interface: I, config: Config) -> Self {
        Self {
            interface,
            config,
            is_display_on: false,
        }
    }

    /// Perform hardware reset, software reset, and initialization
    pub fn reset<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Error<I>> {
        self.interface.reset(delay);
        self.send_command(SOFT_RESET)?;
        self.interface.busy_wait(delay).map_err(Error::Interface)?;
        self.init(delay)
    }

    /// Initialize the controller with configuration
    fn init<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Error<I>> {
        // Temperature sensor
        self.send_command(TEMP_SENSOR_CONTROL)?;
        self.send_data(&[self.config.temp_sensor_control])?;

        // Booster soft start
        self.send_command(BOOSTER_SOFT_START)?;
        let booster_data = self.config.booster_soft_start;
        self.send_data(&booster_data)?;

        // Driver output control
        let rows = self.config.dimensions.rows;
        self.send_command(DRIVER_OUTPUT_CONTROL)?;
        self.send_data(&[
            ((rows - 1) % 256) as u8,
            ((rows - 1) / 256) as u8,
            self.config.gate_scanning,
        ])?;

        // Border waveform
        self.send_command(BORDER_WAVEFORM)?;
        self.send_data(&[self.config.border_waveform])?;

        // VCOM
        self.send_command(WRITE_VCOM)?;
        self.send_data(&[self.config.vcom])?;

        // Clear RAM to white
        self.clear_ram(delay)?;

        Ok(())
    }

    /// Clear display RAM to white
    fn clear_ram<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Error<I>> {
        // Clear BW RAM
        self.send_command(AUTO_WRITE_BW_RAM)?;
        self.send_data(&[0xF7])?;
        self.interface.busy_wait(delay).map_err(Error::Interface)?;

        // Clear RED RAM
        self.send_command(AUTO_WRITE_RED_RAM)?;
        self.send_data(&[0xF7])?;
        self.interface.busy_wait(delay).map_err(Error::Interface)?;

        Ok(())
    }

    /// Update display with user-provided buffers
    ///
    /// # Arguments
    ///
    /// * `black_buffer` - Black/white pixel data (0=black, 1=white)
    /// * `red_buffer` - Red pixel data (0=use BW, 1=red)
    /// * `delay` - Delay implementation
    pub fn update<D: DelayNs>(
        &mut self,
        black_buffer: &[u8],
        red_buffer: &[u8],
        delay: &mut D,
    ) -> Result<(), Error<I>> {
        let expected_size = self.config.dimensions.buffer_size();

        if black_buffer.len() < expected_size {
            return Err(Error::BufferTooSmall {
                required: expected_size,
                provided: black_buffer.len(),
            });
        }
        if red_buffer.len() < expected_size {
            return Err(Error::BufferTooSmall {
                required: expected_size,
                provided: red_buffer.len(),
            });
        }

        // Set full screen area
        self.set_ram_area(
            0,
            0,
            self.config.dimensions.cols,
            self.config.dimensions.rows,
        )?;

        // Write BW RAM
        self.send_command(WRITE_RAM_BW)?;
        self.send_data(&black_buffer[..expected_size])?;

        // Write RED RAM
        self.send_command(WRITE_RAM_RED)?;
        self.send_data(&red_buffer[..expected_size])?;

        // Refresh
        self.refresh(delay, false)
    }

    /// Full refresh with all pixels
    pub fn full_refresh<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Error<I>> {
        // Bypass RED RAM for full refresh
        self.send_command(DISPLAY_UPDATE_CTRL1)?;
        self.send_data(&[CTRL1_BYPASS_RED])?;

        self.refresh(delay, false)
    }

    /// Refresh the display
    fn refresh<D: DelayNs>(&mut self, delay: &mut D, turn_off: bool) -> Result<(), Error<I>> {
        let mut display_mode: u8 = 0x00;

        // Power on if needed
        if !self.is_display_on {
            display_mode |= 0xC0; // CLOCK_ON | ANALOG_ON
        }

        if turn_off {
            display_mode |= 0x03; // ANALOG_OFF | CLOCK_OFF
            self.is_display_on = false;
        } else {
            self.is_display_on = true;
        }

        // Set refresh mode
        display_mode |= 0x34; // TEMP_LOAD | LUT_LOAD | DISPLAY_START

        self.send_command(DISPLAY_UPDATE_CTRL2)?;
        self.send_data(&[display_mode])?;

        self.send_command(MASTER_ACTIVATION)?;

        self.interface.busy_wait(delay).map_err(Error::Interface)?;

        Ok(())
    }

    /// Enter deep sleep mode
    pub fn deep_sleep<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Error<I>> {
        if self.is_display_on {
            // Power down first
            self.send_command(DISPLAY_UPDATE_CTRL1)?;
            self.send_data(&[CTRL1_BYPASS_RED])?;

            self.send_command(DISPLAY_UPDATE_CTRL2)?;
            self.send_data(&[0x03])?; // Power down

            self.send_command(MASTER_ACTIVATION)?;
            self.interface.busy_wait(delay).map_err(Error::Interface)?;

            self.is_display_on = false;
        }

        // Enter deep sleep
        self.send_command(DEEP_SLEEP)?;
        self.send_data(&[0x01])?;

        Ok(())
    }

    /// Load custom LUT (112 bytes for SSD1677)
    pub fn load_lut(&mut self, lut: &[u8]) -> Result<(), Error<I>> {
        self.send_command(WRITE_LUT)?;
        self.send_data(lut)?;

        Ok(())
    }

    /// Set RAM area for partial updates
    fn set_ram_area(&mut self, x: u16, y: u16, w: u16, h: u16) -> Result<(), Error<I>> {
        self.send_command(DATA_ENTRY_MODE)?;
        self.send_data(&[self.config.data_entry_mode])?;

        // X range
        let x_start = x;
        let x_end = x + w - 1;
        self.send_command(SET_RAM_X_RANGE)?;
        self.send_data(&[
            (x_start % 256) as u8,
            (x_start / 256) as u8,
            (x_end % 256) as u8,
            (x_end / 256) as u8,
        ])?;

        // Y range (with gate reversal)
        let y_reversed = self.config.dimensions.rows - y - h;
        let y_start = y_reversed + h - 1;
        let y_end = y_reversed;

        self.send_command(SET_RAM_Y_RANGE)?;
        self.send_data(&[
            (y_start % 256) as u8,
            (y_start / 256) as u8,
            (y_end % 256) as u8,
            (y_end / 256) as u8,
        ])?;

        // Set counters
        self.send_command(SET_RAM_X_COUNTER)?;
        self.send_data(&[(x % 256) as u8, (x / 256) as u8])?;

        self.send_command(SET_RAM_Y_COUNTER)?;
        self.send_data(&[(y_start % 256) as u8, (y_start / 256) as u8])?;

        Ok(())
    }

    /// Send a command to the display controller
    fn send_command(&mut self, cmd: u8) -> Result<(), Error<I>> {
        self.interface.send_command(cmd).map_err(Error::Interface)
    }

    /// Send data to the display controller
    fn send_data(&mut self, data: &[u8]) -> Result<(), Error<I>> {
        self.interface.send_data(data).map_err(Error::Interface)
    }

    /// Get display dimensions
    pub fn dimensions(&self) -> &crate::config::Dimensions {
        &self.config.dimensions
    }

    /// Get display rotation
    pub fn rotation(&self) -> crate::config::Rotation {
        self.config.rotation
    }

    /// Access the underlying configuration
    pub fn config(&self) -> &Config {
        &self.config
    }
}
