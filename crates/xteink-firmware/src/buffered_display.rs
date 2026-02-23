extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

pub struct BufferedDisplay {
    buffer: Vec<u8>,
}

impl BufferedDisplay {
    const NATIVE_WIDTH: u32 = 800;
    const NATIVE_HEIGHT: u32 = 480;
    const NATIVE_WIDTH_BYTES: usize = 100;
    const BUFFER_SIZE: usize = 100 * 480;
    const PORTRAIT_WIDTH: u32 = 480;
    const PORTRAIT_HEIGHT: u32 = 800;

    pub fn new() -> Self {
        Self {
            buffer: vec![0xFF; Self::BUFFER_SIZE],
        }
    }

    pub fn clear(&mut self) {
        self.buffer.fill(0xFF);
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: BinaryColor) {
        if x >= Self::PORTRAIT_WIDTH || y >= Self::PORTRAIT_HEIGHT {
            return;
        }
        let native_x = y;
        let native_y = (Self::PORTRAIT_WIDTH - 1) - x;
        let byte_index = (native_y as usize * Self::NATIVE_WIDTH_BYTES) + (native_x as usize / 8);
        let bit_index = 7 - (native_x % 8);

        if color == BinaryColor::On {
            self.buffer[byte_index] &= !(1 << bit_index);
        } else {
            self.buffer[byte_index] |= 1 << bit_index;
        }
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }
}

impl DrawTarget for BufferedDisplay {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            self.set_pixel(point.x as u32, point.y as u32, color);
        }
        Ok(())
    }

    fn clear(&mut self, color: BinaryColor) -> Result<(), Self::Error> {
        let fill_byte = if color == BinaryColor::On { 0x00 } else { 0xFF };
        self.buffer.fill(fill_byte);
        Ok(())
    }
}

impl OriginDimensions for BufferedDisplay {
    fn size(&self) -> Size {
        Size::new(Self::PORTRAIT_WIDTH, Self::PORTRAIT_HEIGHT)
    }
}

impl Default for BufferedDisplay {
    fn default() -> Self {
        Self::new()
    }
}
