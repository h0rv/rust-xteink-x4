use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::PinDriver,
    peripherals::Peripherals,
    spi::{config::Config, SpiDeviceDriver, SpiDriver, SpiDriverConfig},
};

use ssd1677::{RefreshMode, Ssd1677};
use xteink_ui::App;

fn main() {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();

    // SPI bus (shared pins)
    let spi = SpiDriver::new(
        peripherals.spi2,
        peripherals.pins.gpio8,       // SCLK
        peripherals.pins.gpio10,      // MOSI
        Some(peripherals.pins.gpio7), // MISO
        &SpiDriverConfig::default(),
    )
    .unwrap();

    // SPI device for display
    let spi_device = SpiDeviceDriver::new(
        &spi,
        Some(peripherals.pins.gpio21), // CS
        &Config::default(),
    )
    .unwrap();

    // Control pins
    let dc = PinDriver::output(peripherals.pins.gpio4).unwrap();
    let rst = PinDriver::output(peripherals.pins.gpio5).unwrap();
    let busy = PinDriver::input(peripherals.pins.gpio6).unwrap();

    let mut delay = FreeRtos;

    let mut display = Ssd1677::new(spi_device, dc, rst, busy);

    display.reset_display(&mut delay);
    display.init(&mut delay);
    display.refresh_display(RefreshMode::Full, false, &mut delay);

    // Draw something

    let app = App::new();

    app.render(&mut display);

    display.write_buffer();
    display.refresh_display(RefreshMode::Full, false, &mut delay);

    log::info!("Done!");
}
