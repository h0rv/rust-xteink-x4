# Xteink X4 Firmware Plan

## The Goal

Build an e-reader firmware in Rust. Learn embedded development. Contribute a reusable display driver to the ecosystem.

## Hardware Specs

```
CPU:          ESP32-C3 (RISC-V, single core, 160MHz)
RAM:          400KB SRAM (327KB usable)
              128MB external PSRAM exists but NOT memory-mapped
              (ESP32-C3 lacks hardware PSRAM support - software SPI only)
Flash:        16MB
Storage:      microSD, up to 512GB (ships with 32GB)
Display:      4.3" e-ink, 800×480 native (portrait: 480×800), 220 PPI, B&W
              Controller: SSD1677 (GDEQ0426T82 panel)
              Physical: 69 × 114 mm
Buttons:      7 total (see pin mapping)
Battery:      650 mAh Li-ion (~14 days @ 1-3 hrs/day)
Connectivity: WiFi 2.4GHz, Bluetooth
Port:         USB-C
Formats:      EPUB, TXT, JPG, BMP
No:           Touchscreen, frontlight, 3rd party apps
```

**Pin mapping:**
```
SPI Bus (shared):
  SCLK=8, MOSI=10, MISO=7

Display:
  CS=21, DC=4, RST=5, BUSY=6

SD Card:
  CS=12

Buttons (ADC resistor ladder):
  GPIO1: Back (~3470), Confirm (~2655), Left (~1470), Right (~3)
  GPIO2: VolumeUp (~2205), VolumeDown (~3)
  GPIO3: Power (digital, active LOW)

Battery:
  GPIO0 (voltage divider 2×10K, reads half voltage)

USB Detection:
  GPIO20
```

---

## Learn First

Don't skip this. Embedded Rust has concepts you won't know from regular Rust.

**Read these (in order):**

1. [The Embedded Rust Book](https://docs.rust-embedded.org/book/) - Chapters 1-5, 7
   - Ownership of peripherals, HAL traits, no_std basics

2. [ESP Rust Book](https://docs.esp-rs.org/book/) - Chapters 1-4
   - ESP-IDF vs bare metal, toolchain setup, std on ESP32

3. [embedded-hal docs](https://docs.rs/embedded-hal/latest/embedded_hal/)
   - Skim trait definitions: `SpiDevice`, `OutputPin`, `InputPin`, `DelayNs`

**Reference when stuck:**

- [esp-idf-hal examples](https://github.com/esp-rs/esp-idf-hal/tree/master/examples) - SPI, GPIO, ADC
- [esp-idf-svc examples](https://github.com/esp-rs/esp-idf-svc/tree/master/examples) - Higher-level APIs
- [epd-waveshare source](https://github.com/caemor/epd-waveshare) - E-ink driver patterns (different chip, same ideas)

**Blogs worth reading:**

- [James Munns (OneVariable)](https://jamesmunns.com/) - Embedded Rust patterns
- [Ferrous Systems blog](https://ferrous-systems.com/blog/) - Deep dives
- [Embassy docs](https://embassy.dev/book/) - If you want async later

**Community:**

- [ESP-RS Matrix](https://matrix.to/#/#esp-rs:matrix.org) - Fast answers for ESP32 questions
- [Rust Embedded Matrix](https://matrix.to/#/#rust-embedded:matrix.org) - General embedded

**Video (optional):**

- [Ferrous Systems - Embedded Rust on ESP32](https://www.youtube.com/watch?v=TOAynddiu5M)
- [Low Level Learning](https://www.youtube.com/@LowLevelLearning) - General embedded concepts

---

## Critical Path

Everything else is blocked until the display works. That's the first and hardest task.

```
Display Driver → Button Input → SD Card → UI → Polish
     ↑
  You are here
```

## Phase 1: Display Driver (SSD1677)

This is the make-or-break phase. If SPI doesn't talk to the display, nothing else matters.

### 1.1 Prove SPI Works

Configure pins, send bytes, verify signals. No display output yet.

```
Pins:
- SPI: SCLK=8, MOSI=10, MISO=7
- Display: CS=21, DC=4, RST=5, BUSY=6
```

Test: Toggle reset, read busy pin. If busy pin responds, SPI is working.

### 1.2 Port Init Sequence

Find `GxEPD2_426_GDEQ0426T82.cpp` from original crosspoint firmware. Translate the init commands to Rust.

Don't understand every command yet. Just port it. Understanding comes later.

Test: Display shows *anything* (even garbage = progress).

### 1.3 Full Screen Refresh

Write a 48KB buffer (800×480÷8 bytes) of all 0xFF. Trigger refresh.

Test: Screen turns white. That's the win condition for phase 1.

### 1.4 Implement DrawTarget

Wire up `embedded-graphics` trait. Your simulator UI code should now compile against the real display.

```rust
impl DrawTarget for Ssd1677 {
    type Color = BinaryColor;
    // ...
}
```

Test: Crosshair demo from simulator runs on hardware.

### 1.5 Publish Crate

Extract driver to standalone `ssd1677` crate. Publish to crates.io. First Rust driver for this chip - that's your community contribution.

---

## Phase 2: Button Input

ADC resistor ladder on GPIO1/GPIO2, digital input on GPIO3.

### 2.1 Read Raw ADC

Print ADC values to serial. Press each button, note the voltage ranges.

### 2.2 Map to Buttons

Threshold detection + debouncing (50ms). Wire into your existing `Button` enum.

Test: Button presses show up in serial log.

---

## Phase 3: SD Card

SPI device on CS=12. FAT32 filesystem.

### 3.1 Init SD Card

Use `embedded-sdmmc` crate. Mount filesystem.

### 3.2 List Files

Scan for .txt files. Print to serial.

Test: Insert SD with test.txt, see it listed.

---

## Phase 4: Real UI

Now you have display + buttons + storage. Build the actual e-reader.

### 4.1 Menu Screen

List files from SD. Navigate with buttons.

### 4.2 Text Reader

Load file, wrap text, paginate. Page turn with left/right.

### 4.3 State Machine

Menu ↔ Reader transitions. Remember last position.

---

## Phase 5: Power

### 5.1 Deep Sleep

GPIO wakeup on power button. Save state before sleeping.

### 5.2 Battery Monitor

ADC read, voltage divider compensation, percentage calculation.

---

## Resources

**Datasheets:**
- SSD1677: https://www.good-display.com/companyfile/101.html
- ESP32-C3: https://www.espressif.com/sites/default/files/documentation/esp32-c3_technical_reference_manual_en.pdf

**Reference Code:**
- GxEPD2 (C++): https://github.com/ZinggJM/GxEPD2
- epd-waveshare (Rust, different chips): https://github.com/caemor/epd-waveshare

**Crates:**
- `esp-idf-svc` - ESP32 HAL
- `embedded-graphics` - Drawing primitives
- `embedded-sdmmc` - SD card filesystem
- `embedded-hal` - Hardware abstraction traits

---

## Debug Tips

**SPI not working?**
- Check pin numbers match your board
- Try lower clock speed (1MHz)
- Verify CS goes low during transfer

**Display stays blank?**
- Check BUSY pin - should go low during refresh
- Verify reset sequence timing
- Compare init commands byte-by-byte with GxEPD2

**Garbage on screen?**
- Check byte order (MSB vs LSB)
- Verify buffer size matches display RAM
- Try inverting colors (0xFF vs 0x00 for white)

---

## Done When

- [ ] Display shows UI from `xteink-ui` crate
- [ ] Buttons navigate menus
- [ ] Can read .txt files from SD card
- [ ] Device sleeps and wakes on button press
- [ ] `ssd1677` crate published on crates.io
