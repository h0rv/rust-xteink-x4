use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::PinDriver,
    peripherals::Peripherals,
    spi::{config::Config, SpiDeviceDriver, SpiDriver, SpiDriverConfig},
};

use ssd1677::{RefreshMode, Ssd1677};
use xteink_ui::App;

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Starting firmware...");

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

    log::info!("Resetting display...");
    display.reset_display(&mut delay);

    log::info!("Initializing display...");
    display.init(&mut delay);

    // Draw UI
    let app = App::new();
    app.render(&mut display).ok();

    // Write buffer to display and refresh once
    log::info!("Writing buffer and refreshing...");
    display.write_buffer();
    display.refresh_display(RefreshMode::Full, false, &mut delay);

    log::info!("Done!");
}
