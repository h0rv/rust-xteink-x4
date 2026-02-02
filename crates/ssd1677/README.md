# SSD1677 E-Paper Display Driver

[![Crates.io](https://img.shields.io/crates/v/ssd1677)](https://crates.io/crates/ssd1677)
[![Docs.rs](https://docs.rs/ssd1677/badge.svg)](https://docs.rs/ssd1677)
[![License](https://img.shields.io/crates/l/ssd1677)](LICENSE-MIT)

A `no_std` driver for the **SSD1677** e-paper display controller, supporting displays up to **800x480 pixels** with tri-color (black/white/red) support.

## Features

- `no_std` compatible - suitable for bare-metal embedded systems
- `embedded-hal` v1.0 support
- `embedded-graphics` integration (optional, enabled by default)
- Full and fast refresh modes
- Custom Look-Up Table (LUT) support for custom waveforms
- Display rotation support (0°, 90°, 180°, 270°)
- Type-safe configuration builder
- Efficient buffer management

## Supported Displays

The SSD1677 controller supports various e-paper display sizes:

- 4.2" (400x300)
- 5.83" (648x480)
- 7.5" (800x480)
- And others up to 800x480 pixels

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
ssd1677 = "0.1.0"
embedded-hal = "1.0.0"
```

Basic usage example:

```rust
use ssd1677::{Builder, Dimensions, Display, Interface, Rotation};
use embedded_hal::delay::DelayNs;

// Create hardware interface (SPI + GPIO pins)
let interface = Interface::new(spi, dc_pin, rst_pin, busy_pin);

// Configure display dimensions and rotation
let config = Builder::new()
    .dimensions(Dimensions::new(480, 800).expect("valid dimensions"))
    .rotation(Rotation::Rotate0)
    .build()
    .expect("valid configuration");

// Create display driver and initialize
let mut display = Display::new(interface, config);
display.reset(&mut delay).expect("reset failed");

// Update display with buffers
let black_buffer = vec![0xFF; buffer_size]; // All white
let red_buffer = vec![0x00; buffer_size];   // No red
display.update(&black_buffer, &red_buffer, &mut delay)
    .expect("update failed");
```

### Using with embedded-graphics

Enable the `graphics` feature (enabled by default):

```toml
[dependencies]
ssd1677 = { version = "0.1.0", features = ["graphics"] }
embedded-graphics = "0.8"
```

```rust
use ssd1677::{Builder, Dimensions, Display, Interface, Rotation, GraphicDisplay, Color};
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text,
};

// Setup display...
let display = Display::new(interface, config);

// Create graphic display with buffers
let mut graphic_display = GraphicDisplay::new(
    display,
    vec![0u8; buffer_size],  // Black buffer
    vec![0u8; buffer_size],  // Red buffer
);

// Draw using embedded-graphics
Text::new("Hello, E-Paper!", Point::new(10, 20), 
    MonoTextStyle::new(&FONT_6X10, BinaryColor::On))
    .draw(&mut graphic_display)?;

// Update display
graphic_display.update(&mut delay)?;
```

## Hardware Interface

The SSD1677 requires:

### Pin Connections

| SSD1677 Pin | MCU Pin | Description |
|------------|---------|-------------|
| VCC | 3.3V | Power supply |
| GND | GND | Ground |
| DIN | MOSI | SPI Data In |
| CLK | SCK | SPI Clock |
| CS | GPIO (CS) | SPI Chip Select |
| DC | GPIO | Data/Command select |
| RST | GPIO | Hardware reset |
| BUSY | GPIO (Input) | Busy status (active high) |

### Wiring Diagram

```
       MCU                    SSD1677 Display
    ┌─────────┐             ┌───────────────┐
    │         │             │               │
    │    MOSI ├─────────────┤ DIN           │
    │    SCK  ├─────────────┤ CLK           │
    │    CS   ├─────────────┤ CS            │
    │    GPIO ├─────────────┤ DC            │
    │    GPIO ├─────────────┤ RST           │
    │    GPIO ├─────────────┤ BUSY          │
    │         │             │               │
    │    3.3V ├─────────────┤ VCC           │
    │    GND  ├─────────────┤ GND           │
    │         │             │               │
    └─────────┘             └───────────────┘
```

## Configuration

### Display Dimensions

Dimensions must meet these constraints:
- Rows: 1 to 800 (height)
- Columns: 8 to 480, must be multiple of 8 (width)

```rust
use ssd1677::Dimensions;

// 7.5" display (800x480)
let dims = Dimensions::new(800, 480)?;

// 5.83" display (648x480)
let dims = Dimensions::new(648, 480)?;

// 4.2" display (400x300)
let dims = Dimensions::new(400, 300)?;
```

### Advanced Configuration

```rust
use ssd1677::Builder;

let config = Builder::new()
    .dimensions(dims)
    .rotation(Rotation::Rotate90)
    .vcom(0x3C)                    // VCOM voltage
    .border_waveform(0x01)         // Border waveform
    .booster_soft_start([0xAE, 0xC7, 0xC3, 0xC0, 0x40])
    .build()?;
```

### Refresh Modes

```rust
// Full refresh - clears entire display, best quality
fn full_refresh<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Error<I>>;

// Partial update from buffers
fn update<D: DelayNs>(
    &mut self,
    black_buffer: &[u8],
    red_buffer: &[u8],
    delay: &mut D
) -> Result<(), Error<I>>;
```

### Custom LUT

For custom waveforms (e.g., grayscale or fast refresh):

```rust
// Load custom 112-byte LUT
const CUSTOM_LUT: [u8; 112] = [/* your waveform data */];
display.load_lut(&CUSTOM_LUT)?;
```

## Examples

See the [examples/](examples/) directory for complete examples including:
- Basic initialization and display update
- Graphics drawing with embedded-graphics
- Custom LUT usage
- Rotation handling

## Resources

- [SSD1677 Datasheet](https://www.solumco.com/files/SSD1677.pdf)
- [embedded-hal](https://github.com/rust-embedded/embedded-hal) - Hardware abstraction layer
- [embedded-graphics](https://github.com/embedded-graphics/embedded-graphics) - 2D graphics library
- Community SDK Reference: [open-x4-epaper](https://github.com/open-x4-epaper/community-sdk)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
