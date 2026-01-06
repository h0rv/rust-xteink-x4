mod command;
mod lut;

use core::convert::Infallible;

use command::*;

use embedded_graphics_core::{
    Pixel,
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::BinaryColor,
};
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

pub const DISPLAY_WIDTH: usize = 480;
pub const DISPLAY_HEIGHT: usize = 800;
pub const DISPLAY_BUFFER_SIZE: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT / 8;

pub enum RefreshMode {
    Full, // Full refresh with complete waveform
    Half, // Half refresh (1720ms) - balanced quality and speed
    Fast, // Fast refresh using custom LUT
}

pub struct Ssd1677<SPI, DC, RST, BUSY> {
    spi: SPI,
    dc: DC,
    rst: RST,
    busy: BUSY,

    is_display_on: bool,
    custom_lut_active: bool,
    buffer: [u8; DISPLAY_BUFFER_SIZE],
}

impl<SPI, DC, RST, BUSY> Ssd1677<SPI, DC, RST, BUSY>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
{
    pub fn new(spi: SPI, dc: DC, rst: RST, busy: BUSY) -> Self {
        Self {
            spi,
            dc,
            rst,
            busy,
            is_display_on: false,
            custom_lut_active: false,
            buffer: [0; DISPLAY_BUFFER_SIZE],
        }
    }

    fn send_command(&mut self, command: u8) {
        send_command(&mut self.spi, &mut self.dc, command)
    }

    fn send_data(&mut self, data: &[u8]) {
        send_data(&mut self.spi, &mut self.dc, data)
    }

    fn send_byte(&mut self, byte: u8) {
        self.send_data(&[byte]);
    }

    fn wait_while_busy(&mut self) {
        while self.busy.is_low().unwrap() {}
    }

    pub fn soft_reset(&mut self) {
        self.send_command(SOFT_RESET);
        self.wait_while_busy()
    }

    pub fn temperature_sensor(&mut self) {
        self.send_command(TEMP_SENSOR_CONTROL);
        self.send_byte(0x80);
    }

    pub fn booster_soft_start(&mut self) {
        self.send_command(BOOSTER_SOFT_START);
        self.send_byte(0xAE);
        self.send_byte(0xC7);
        self.send_byte(0xC3);
        self.send_byte(0xC0);
        self.send_byte(0x40);
    }

    pub fn driver_output_control(&mut self) {
        self.send_command(DRIVER_OUTPUT_CONTROL);
        // set display height
        self.send_byte(((DISPLAY_HEIGHT - 1) % 256) as u8); // low byte
        self.send_byte(((DISPLAY_HEIGHT - 1) / 256) as u8); // high byte
        // scan direction
        self.send_byte(0x02);
    }

    pub fn border_waveform(&mut self) {
        self.send_command(BORDER_WAVEFORM);
        self.send_byte(0x01);
    }

    pub fn set_ram_area(&mut self, x: usize, y: usize, width: usize, height: usize) {
        const DATA_ENTRY_X_INC_Y_DEC: u8 = 0x01;

        // Reverse Y coordinate (gates are reversed on this display)
        let y = DISPLAY_HEIGHT - y - height;

        // Set data entry mode (X increment, Y decrement for reversed gates)
        self.send_command(DATA_ENTRY_MODE);
        self.send_byte(DATA_ENTRY_X_INC_Y_DEC);

        // // Set RAM X address range (start, end) - X is in PIXELS
        self.send_command(SET_RAM_X_RANGE);
        self.send_byte((x % 256) as u8); // start low byte
        self.send_byte((x / 256) as u8); // start high byte
        self.send_byte(((x + width - 1) % 256) as u8); // end low byte
        self.send_byte(((x + width - 1) / 256) as u8); // end high byte

        // // Set RAM Y address range (start, end) - Y is in PIXELS
        self.send_command(SET_RAM_Y_RANGE);
        self.send_byte(((y + height - 1) % 256) as u8); // start low byte
        self.send_byte(((y + height - 1) / 256) as u8); // start high byte
        self.send_byte((y % 256) as u8); // end low byte
        self.send_byte((y / 256) as u8); // end high byte

        // // Set RAM X address counter - X is in PIXELS
        self.send_command(SET_RAM_X_COUNTER);
        self.send_byte((x % 256) as u8); // low byte
        self.send_byte((x / 256) as u8); // high byte

        // // Set RAM Y address counter - Y is in PIXELS
        self.send_command(SET_RAM_Y_COUNTER);
        self.send_byte(((y + height - 1) % 256) as u8); // low byte
        self.send_byte(((y + height - 1) / 256) as u8); // high byte
    }

    pub fn clear_ram(&mut self) {
        self.send_command(AUTO_WRITE_BW_RAM);
        self.send_byte(0xF7);
        self.wait_while_busy();

        self.send_command(AUTO_WRITE_RED_RAM);
        self.send_byte(0xF7);
        self.wait_while_busy();
    }

    pub fn write_buffer(&mut self) {
        self.send_command(WRITE_RAM_BW);
        send_data(&mut self.spi, &mut self.dc, &self.buffer);
    }

    pub fn init(&mut self) {
        self.soft_reset();
        self.temperature_sensor();
        self.booster_soft_start();
        self.driver_output_control();
        self.border_waveform();
        self.clear_ram();
    }

    pub fn reset_display(&mut self, delay: &mut impl DelayNs) {
        self.rst.set_high().ok();
        delay.delay_ms(20);
        self.rst.set_low().ok();
        delay.delay_ms(2);
        self.rst.set_high().ok();
        delay.delay_ms(20);
    }

    pub fn refresh_display(&mut self, refresh_mode: RefreshMode, turn_off_display: bool) {
        self.send_command(DISPLAY_UPDATE_CTRL1);
        let data = match refresh_mode {
            RefreshMode::Full | RefreshMode::Half => CTRL1_BYPASS_RED,
            RefreshMode::Fast => CTRL1_NORMAL,
        };
        self.send_byte(data);

        // bit | hex | name                    | effect
        // ----+-----+--------------------------+-------------------------------------------
        // 7   | 80  | CLOCK_ON                | Start internal oscillator
        // 6   | 40  | ANALOG_ON               | Enable analog power rails (VGH/VGL drivers)
        // 5   | 20  | TEMP_LOAD               | Load temperature (internal or I2C)
        // 4   | 10  | LUT_LOAD                | Load waveform LUT
        // 3   | 08  | MODE_SELECT             | Mode 1/2
        // 2   | 04  | DISPLAY_START           | Run display
        // 1   | 02  | ANALOG_OFF_PHASE        | Shutdown step 1 (undocumented)
        // 0   | 01  | CLOCK_OFF               | Disable internal oscillator

        let mut display_mode = 0x00;

        if !self.is_display_on {
            self.is_display_on = true;
            display_mode |= 0xC0; // Set CLOCK_ON and ANALOG_ON bits
        }

        if turn_off_display {
            self.is_display_on = false;
            display_mode |= 0x03; // Set ANALOG_OFF_PHASE and CLOCK_OFF bits
        }

        display_mode |= match refresh_mode {
            RefreshMode::Full => 0x34,
            RefreshMode::Half => {
                self.send_command(WRITE_TEMP);
                self.send_byte(0x5A);
                0xD4
            }
            RefreshMode::Fast => {
                if self.custom_lut_active {
                    0x0C
                } else {
                    0x1C
                }
            }
        };

        // Power on and refresh display
        self.send_command(DISPLAY_UPDATE_CTRL2);
        self.send_byte(display_mode);

        self.send_command(MASTER_ACTIVATION);

        self.wait_while_busy();
    }

    pub fn deep_sleep(&mut self) {
        // First, power down the display properly
        // This shuts down the analog power rails and clock
        if self.is_display_on {
            self.send_command(DISPLAY_UPDATE_CTRL1);
            self.send_byte(CTRL1_BYPASS_RED); // Normal mode

            self.send_command(DISPLAY_UPDATE_CTRL2);
            self.send_byte(0x03); // Set ANALOG_OFF_PHASE (bit 1) and CLOCK_OFF (bit 0)

            self.send_command(MASTER_ACTIVATION);

            // Wait for the power-down sequence to complete
            self.wait_while_busy();

            self.is_display_on = false;
        }

        // Now enter deep sleep mode
        self.send_command(DEEP_SLEEP);
        self.send_byte(0x01); // Enter deep sleep
    }

    pub fn load_lut(&mut self, lut: &[u8; 112]) {
        self.send_command(WRITE_LUT);
        self.send_data(lut);
        self.custom_lut_active = true;
    }

    fn in_width(&self, x: i32) -> bool {
        0 <= x && x < DISPLAY_WIDTH as i32
    }

    fn in_height(&self, y: i32) -> bool {
        0 <= y && y < DISPLAY_HEIGHT as i32
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        self.in_width(x) && self.in_height(y)
    }

    fn get_buffer_location(&self, x: i32, y: i32) -> (i32, u8) {
        let pixel_index = y * DISPLAY_WIDTH as i32 + x;

        let pixel_byte = pixel_index / 8;
        let pixel_byte_offset = pixel_index % 8;

        (pixel_byte, pixel_byte_offset as u8)
    }

    fn set_pixel(&mut self, x: i32, y: i32, color: BinaryColor) {
        if !(self.in_bounds(x, y)) {
            return;
        }

        let (byte_index, byte_offset) = self.get_buffer_location(x, y);
        let buffer_byte = self.buffer.get_mut(byte_index as usize).unwrap();

        match color {
            BinaryColor::On => *buffer_byte |= 1 << byte_offset, // +1 or 0 indexed?
            BinaryColor::Off => *buffer_byte &= !(1 << byte_offset), // +1 or 0 indexed?
        }
    }
}

fn send_command<SPI, DC>(spi: &mut SPI, dc: &mut DC, command: u8)
where
    SPI: SpiDevice,
    DC: OutputPin,
{
    dc.set_low().ok();
    spi.write(&[command]).ok();
}

fn send_data<SPI, DC>(spi: &mut SPI, dc: &mut DC, data: &[u8])
where
    SPI: SpiDevice,
    DC: OutputPin,
{
    dc.set_high().ok();
    spi.write(data).ok();
}

impl<SPI, DC, RST, BUSY> OriginDimensions for Ssd1677<SPI, DC, RST, BUSY> {
    fn size(&self) -> Size {
        Size::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32)
    }
}

impl<SPI, DC, RST, BUSY> DrawTarget for Ssd1677<SPI, DC, RST, BUSY>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
{
    type Color = BinaryColor;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            self.set_pixel(point.x, point.y, color);
        }
        Ok(())
    }
}
