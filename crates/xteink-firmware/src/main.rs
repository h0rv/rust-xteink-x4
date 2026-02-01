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

    // SPI bus (shared pins) - configure for 40MHz, Mode 0
    let spi = SpiDriver::new(
        peripherals.spi2,
        peripherals.pins.gpio8,       // SCLK
        peripherals.pins.gpio10,      // MOSI
        Some(peripherals.pins.gpio7), // MISO
        &SpiDriverConfig::default(),
    )
    .unwrap();

    // SPI device for display - 40MHz, Mode 0 (reference implementation settings)
    let spi_config =
        Config::default()
            .baudrate(40.MHz().into())
            .data_mode(embedded_hal::spi::Mode {
                polarity: embedded_hal::spi::Polarity::IdleLow,
                phase: embedded_hal::spi::Phase::CaptureOnFirstTransition,
            });

    let spi_device = SpiDeviceDriver::new(
        &spi,
        Some(peripherals.pins.gpio21), // CS
        &spi_config,
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
