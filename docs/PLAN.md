# Xteink X4 E-ink Reader - Complete Development Plan

## Phase 0: Environment Setup (Day 1 - 2 hours)

### Install Toolchain
```bash
# Core Rust tools
rustup update
rustup component add rust-src

# ESP32 tools
cargo install espup
cargo install cargo-espflash
cargo install ldproxy
cargo install espflash
cargo install probe-rs

# Install ESP Rust toolchain
espup install
source $HOME/export-esp.sh  # Add to ~/.bashrc or ~/.zshrc

# Optional but useful
cargo install cargo-bloat      # Size analysis
cargo install cargo-outdated   # Dependency updates
cargo install cargo-expand     # Macro expansion
```

### Create Project
```bash
cargo install cargo-generate
cargo generate esp-rs/esp-idf-template cargo
# Name: xteink-reader
# MCU: esp32c3
# STD support: true
# ESP-IDF: v5.1 or latest
# Advanced: false

cd xteink-reader
```

### Verify Setup
```bash
cargo build --release
# Should compile successfully (may take 5-10 min first time)
```

### Resources
- **Official Guide**: https://docs.esp-rs.org/book/
- **ESP-IDF API Docs**: https://docs.esp-rs.org/esp-idf-svc/
- **Hardware Reference**: https://www.espressif.com/sites/default/files/documentation/esp32-c3_datasheet_en.pdf

---

## Phase 1: Embedded Rust Fundamentals (Days 2-3 - 8 hours)

### Learning Goals
- Understand `no_std` vs `std` in embedded context
- Master `embedded-hal` traits (foundation of everything)
- Learn ESP-IDF integration model
- Understand memory safety in embedded (no panics, stack vs heap)

### Study Resources

**Primary (4 hours):**
- [The Embedded Rust Book](https://docs.rust-embedded.org/book/) - Chapters 1-5, 7
  - Focus on: Hardware abstractions, I/O, Concurrency
- [ESP Rust Book](https://docs.esp-rs.org/book/) - Chapters 1-4
  - Focus on: ESP-IDF integration, HAL usage

**Secondary (2 hours):**
- [embedded-hal documentation](https://docs.rs/embedded-hal/latest/embedded_hal/)
  - Skim trait definitions: `OutputPin`, `InputPin`, `SpiDevice`, `Delay`
- [ESP-IDF-SVC examples](https://github.com/esp-rs/esp-idf-svc/tree/master/examples)
  - Read: `gpio`, `adc`, `spi_loopback`

**Hands-on (2 hours):**
```rust
// Exercise 1: Blink LED (any GPIO pin)
// Exercise 2: Read digital input with pullup
// Exercise 3: PWM output (fade LED)
```

### Key Concepts Checklist
- [ ] Understand peripheral ownership (Peripherals::take())
- [ ] Know when to use `anyhow` vs `Result<T, E>`
- [ ] Understand blocking vs non-blocking operations
- [ ] Know stack size limits (~4KB per task typical)
- [ ] Understand ESP-IDF FreeRTOS integration

---

## Phase 2: Hardware Bring-up (Days 4-6 - 12 hours)

### Milestone 1: Button Input System (4 hours)

**Goal:** Read all 7 buttons via ADC resistor ladder + digital input

**Implementation Steps:**
1. Configure ADC for GPIO1 and GPIO2 (12-bit, DB_12 attenuation)
2. Implement resistor ladder detection with thresholds
3. Add debouncing (50ms typical)
4. Configure GPIO3 as digital input with pullup
5. Create button state machine

**Code Structure:**
```rust
// src/buttons.rs
pub enum Button {
    None,
    Right,
    Left,
    Confirm,
    Back,
    VolumeUp,
    VolumeDown,
    Power,
}

pub struct ButtonReader {
    adc1_channel: AdcChannelDriver<'static, ...>,
    adc2_channel: AdcChannelDriver<'static, ...>,
    power_pin: PinDriver<'static, ...>,
    last_state: Button,
    last_change: Instant,
}

impl ButtonReader {
    pub fn new(...) -> Result<Self>;
    pub fn read(&mut self) -> Button;
    pub fn read_debounced(&mut self) -> Option<Button>;
}
```

**Resources:**
- ESP32-C3 ADC characteristics: [Technical Reference Manual](https://www.espressif.com/sites/default/files/documentation/esp32-c3_technical_reference_manual_en.pdf) - Chapter 28
- esp-idf-hal ADC examples: https://github.com/esp-rs/esp-idf-hal/tree/master/examples

**Testing:**
- Press each button, verify ADC values match documented ranges
- Implement serial logging: `log::info!("Button: {:?} ADC: {}", button, raw_value)`

### Milestone 2: Battery Monitoring (2 hours)

**Goal:** Read battery voltage and calculate percentage

**Implementation Steps:**
1. Configure ADC for battery pin (GPIO0 assumed - verify schematic)
2. Implement voltage divider compensation (×2 multiplier)
3. Use ESP ADC calibration API
4. Map voltage to percentage (3.0V = 0%, 4.2V = 100%)

**Code Structure:**
```rust
// src/battery.rs
pub struct BatteryMonitor {
    adc_channel: AdcChannelDriver<'static, ...>,
    divider_multiplier: f32,
}

impl BatteryMonitor {
    pub fn new(...) -> Result<Self>;
    pub fn read_voltage(&mut self) -> Result<f32>;  // Returns volts
    pub fn read_percentage(&mut self) -> Result<u8>; // Returns 0-100
}
```

**Resources:**
- ADC calibration: https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/peripherals/adc_calibration.html
- Li-ion discharge curves: https://batteryuniversity.com/article/bu-501-basics-about-discharging

**Testing:**
- Compare with multimeter reading (should be ±50mV)
- Test at different battery levels

### Milestone 3: SPI Initialization (3 hours)

**Goal:** Initialize SPI bus for display and SD card

**Implementation Steps:**
1. Configure SPI2 with custom pins (SCLK=8, MOSI=10, MISO=7)
2. Set up 40MHz clock (per original code)
3. Create device drivers for display (CS=21) and SD card (CS=12)
4. Implement CS pin management (only one device active at a time)

**Code Structure:**
```rust
// src/spi_bus.rs
pub struct SharedSpiBus {
    spi_driver: SpiDriver<'static>,
    display_cs: PinDriver<'static, Output>,
    sd_cs: PinDriver<'static, Output>,
}

impl SharedSpiBus {
    pub fn new(...) -> Result<Self>;
    pub fn get_display_device(&mut self) -> SpiDeviceDriver<...>;
    pub fn get_sd_device(&mut self) -> SpiDeviceDriver<...>;
}
```

**Resources:**
- ESP32-C3 SPI: https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/peripherals/spi_master.html
- esp-idf-hal SPI: https://docs.rs/esp-idf-hal/latest/esp_idf_hal/spi/

**Testing:**
- Logic analyzer on SPI pins (optional but recommended)
- SPI loopback test (connect MOSI to MISO temporarily)

### Milestone 4: SD Card Access (3 hours)

**Goal:** Mount SD card and read files

**Implementation Steps:**
1. Initialize SD card via SPI
2. Mount FAT32 filesystem
3. List files in root directory
4. Read a test text file

**Code Structure:**
```rust
// src/storage.rs
pub struct SdStorage {
    volume_mgr: VolumeManager<...>,
    volume: Volume,
}

impl SdStorage {
    pub fn new(spi_device: ...) -> Result<Self>;
    pub fn list_files(&mut self, path: &str) -> Result<Vec<String>>;
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>>;
}
```

**Dependencies:**
```toml
embedded-sdmmc = "0.8"
```

**Resources:**
- embedded-sdmmc docs: https://docs.rs/embedded-sdmmc/latest/embedded_sdmmc/
- Example: https://github.com/rust-embedded-community/embedded-sdmmc-rs/tree/develop/examples

**Testing:**
- Format SD card as FAT32
- Create test.txt with known content
- Verify file reading

---

## Phase 3: E-ink Display Driver (Days 7-12 - 20 hours)

### Background Study (3 hours)

**Must-read documents:**
1. [SSD1677 Datasheet](https://www.good-display.com/companyfile/101.html) - Focus on:
   - Initialization sequence (pg 15-18)
   - Command table (pg 19-25)
   - Timing diagrams (pg 26-30)

2. [GDEQ0426T82 Display Specs](https://www.good-display.com/product/457.html)
   - Resolution: 800×480
   - Memory: 1-bit per pixel (black/white)
   - Buffer size: 800×480/8 = 48,000 bytes

3. **Study existing driver code:**
   - GxEPD2_426_GDEQ0426T82.cpp from original project
   - Note initialization commands, LUT tables, refresh sequences

**Key concepts to understand:**
- Gate/source drivers
- Display RAM organization
- Partial vs full refresh
- LUT (Look-Up Table) for waveforms
- Busy pin handling

### Milestone 5: Basic Display Communication (4 hours)

**Goal:** Send commands to display, verify busy pin works

**Implementation Steps:**
1. Configure display control pins (DC=4, RST=5, BUSY=6)
2. Implement command/data write functions
3. Implement reset sequence
4. Implement busy wait
5. Send initialization sequence

**Code Structure:**
```rust
// src/display/ssd1677.rs
pub struct Ssd1677<SPI> {
    spi: SPI,
    dc: PinDriver<'static, Output>,
    rst: PinDriver<'static, Output>,
    busy: PinDriver<'static, Input>,
}

impl<SPI: SpiDevice> Ssd1677<SPI> {
    pub fn new(...) -> Result<Self>;
    
    fn send_command(&mut self, cmd: u8) -> Result<()>;
    fn send_data(&mut self, data: &[u8]) -> Result<()>;
    fn wait_busy(&self);
    fn reset(&mut self);
    
    pub fn init(&mut self) -> Result<()>;
}
```

**Testing:**
- Verify BUSY pin goes low during operations
- Use logic analyzer to verify SPI commands match datasheet

### Milestone 6: Full Display Refresh (5 hours)

**Goal:** Draw something visible on screen

**Implementation Steps:**
1. Implement buffer management (48KB for full screen)
2. Port initialization LUT from GxEPD2
3. Implement display RAM write
4. Trigger full refresh
5. Wait for completion

**Code Structure:**
```rust
impl<SPI: SpiDevice> Ssd1677<SPI> {
    pub fn clear(&mut self) -> Result<()>;
    pub fn draw_buffer(&mut self, buffer: &[u8]) -> Result<()>;
    pub fn refresh(&mut self) -> Result<()>;
}
```

**Testing:**
- Clear screen to white
- Draw checkerboard pattern
- Display should update (will take 2-3 seconds)

### Milestone 7: Partial Refresh (4 hours)

**Goal:** Update small regions without full refresh

**Implementation Steps:**
1. Implement window setting commands
2. Port partial refresh LUT
3. Implement partial update sequence
4. Optimize for text regions

**Code Structure:**
```rust
impl<SPI: SpiDevice> Ssd1677<SPI> {
    pub fn set_partial_window(&mut self, x: u16, y: u16, w: u16, h: u16);
    pub fn draw_partial(&mut self, buffer: &[u8]) -> Result<()>;
    pub fn refresh_partial(&mut self) -> Result<()>;
}
```

**Resources:**
- Partial refresh explanation: https://www.waveshare.com/wiki/E-Paper_Driver_HAT
- Understanding LUTs: https://www.crystalfontz.com/blog/understanding-e-paper-displays/

**Testing:**
- Update small text area
- Verify refresh time < 500ms
- Check for ghosting (common issue)

### Milestone 8: embedded-graphics Integration (4 hours)

**Goal:** Use standard drawing API

**Implementation Steps:**
1. Implement `DrawTarget` trait
2. Add pixel-level drawing
3. Test with shapes and text

**Dependencies:**
```toml
embedded-graphics = "0.8"
```

**Code Structure:**
```rust
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    Drawable, DrawTarget,
};

impl DrawTarget for Ssd1677<SPI> {
    type Color = BinaryColor;
    type Error = DisplayError;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>;

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) 
        -> Result<(), Self::Error>;
}
```

**Resources:**
- embedded-graphics book: https://docs.rs/embedded-graphics/latest/embedded_graphics/
- Example drivers: https://github.com/embedded-graphics/embedded-graphics/tree/master/examples

**Testing:**
```rust
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    text::Text,
    primitives::{Circle, Rectangle, PrimitiveStyle},
};

// Draw shapes
Circle::new(Point::new(100, 100), 50)
    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
    .draw(&mut display)?;

// Draw text
let style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
Text::new("Hello Xteink!", Point::new(20, 30), style)
    .draw(&mut display)?;
```

---

## Phase 4: Application Logic (Days 13-16 - 12 hours)

### Milestone 9: State Machine (3 hours)

**Goal:** Implement UI navigation

**Implementation Steps:**
1. Define application states (Menu, Reading, Settings)
2. Implement state transitions based on buttons
3. Add basic menu rendering

**Code Structure:**
```rust
// src/app.rs
#[derive(Debug, Clone, Copy)]
pub enum AppState {
    Menu,
    Reading { file_index: usize },
    Settings,
}

pub struct App {
    state: AppState,
    files: Vec<String>,
    selected_index: usize,
}

impl App {
    pub fn handle_button(&mut self, button: Button) -> StateChange;
    pub fn render(&self, display: &mut impl DrawTarget);
}
```

### Milestone 10: File Browser (4 hours)

**Goal:** Browse and select books from SD card

**Implementation Steps:**
1. Scan SD card for text/epub files
2. Implement scrollable list UI
3. Add file selection
4. Cache file list

**Testing:**
- Add several .txt files to SD card
- Navigate with UP/DOWN buttons
- Select with CONFIRM

### Milestone 11: Text Rendering (5 hours)

**Goal:** Display book content with pagination

**Implementation Steps:**
1. Load text file into memory (or chunks)
2. Implement text wrapping
3. Add pagination
4. Implement page turn animations (optional)

**Resources:**
- Text wrapping algorithms: https://en.wikipedia.org/wiki/Line_wrap_and_word_wrap
- Consider: https://docs.rs/embedded-text/latest/embedded_text/

**Code Structure:**
```rust
// src/reader.rs
pub struct TextReader {
    content: String,  // Or buffered chunks
    current_page: usize,
    chars_per_page: usize,
}

impl TextReader {
    pub fn load_file(path: &str, storage: &mut SdStorage) -> Result<Self>;
    pub fn next_page(&mut self);
    pub fn prev_page(&mut self);
    pub fn render(&self, display: &mut impl DrawTarget);
}
```

---

## Phase 5: Power Management (Days 17-18 - 6 hours)

### Milestone 12: Deep Sleep (4 hours)

**Goal:** Implement power saving mode

**Implementation Steps:**
1. Configure GPIO wakeup on power button (GPIO3)
2. Implement sleep entry sequence
3. Verify long-press on wakeup (1 second)
4. Save state before sleeping

**Code Structure:**
```rust
// src/power.rs
pub fn enter_deep_sleep() -> ! {
    log::info!("Entering deep sleep");
    
    // Configure wakeup
    unsafe {
        esp_idf_sys::esp_deep_sleep_enable_gpio_wakeup(
            1 << 3,  // GPIO3 mask
            esp_idf_sys::esp_gpio_wakeup_mode_t_ESP_GPIO_WAKEUP_GPIO_LOW,
        );
        esp_idf_sys::esp_deep_sleep_start();
    }
}

pub fn verify_wakeup_long_press() -> bool {
    // Return true if button held for 1+ second
    // Return false and sleep again if released early
}
```

**Resources:**
- ESP32 deep sleep: https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/system/sleep_modes.html

**Testing:**
- Measure current draw (should be <100µA in deep sleep)
- Verify wakeup works
- Test false-wakeup rejection

### Milestone 13: Battery Optimization (2 hours)

**Goal:** Minimize active power consumption

**Implementation:**
1. Reduce SPI clock when possible
2. Sleep display between updates
3. Implement auto-sleep timer (e.g., 5 min)
4. Profile power usage

**Testing:**
- Measure active current (should be <50mA when idle)
- Verify auto-sleep triggers

---

## Phase 6: Polish & Features (Days 19-21 - 8 hours)

### Milestone 14: Error Handling (2 hours)

**Goal:** Graceful error recovery

**Implementation:**
1. Add proper error types for each module
2. Implement error display on screen
3. Add recovery strategies (retry, fallback)

### Milestone 15: Settings Persistence (3 hours)

**Goal:** Save user preferences

**Options:**
- NVS (Non-Volatile Storage) partition in flash
- Config file on SD card

**Implementation:**
```rust
// src/config.rs
#[derive(Serialize, Deserialize)]
pub struct Config {
    brightness: u8,  // If controllable
    auto_sleep_mins: u8,
    last_file: Option<String>,
    last_page: usize,
}

impl Config {
    pub fn load() -> Result<Self>;
    pub fn save(&self) -> Result<()>;
}
```

### Milestone 16: Nice-to-Haves (3 hours)

Pick one or more:
- Battery indicator on screen
- Page progress bar
- Bookmarks
- Font size adjustment
- Clock display

---

## Phase 7: Testing & Refinement (Days 22-25 - 12 hours)

### Integration Testing (4 hours)
- [ ] Test all button combinations
- [ ] Test with large files (>1MB)
- [ ] Test SD card removal/insertion
- [ ] Test low battery behavior
- [ ] Test continuous use (2+ hours)
- [ ] Test deep sleep/wake cycles (100+ times)

### Performance Optimization (4 hours)
- [ ] Profile with `cargo flamegraph` (if supported)
- [ ] Optimize display refresh times
- [ ] Reduce binary size (`cargo bloat --release`)
- [ ] Optimize memory usage

### Bug Fixing (4 hours)
- [ ] Fix any crashes or hangs
- [ ] Address display artifacts
- [ ] Fix button debouncing issues
- [ ] Handle edge cases

---

## Development Best Practices

### Daily Workflow
```bash
# 1. Pull latest dependencies
cargo update

# 2. Build
cargo build --release

# 3. Flash and monitor
cargo espflash flash --release --monitor

# 4. On errors, check sizes
cargo bloat --release --crates
```

### Debugging Techniques

**Serial Logging:**
```rust
log::info!("Button pressed: {:?}", button);
log::warn!("Battery low: {}%", percentage);
log::error!("SD card read failed: {:?}", err);
```

**Panic Handler:**
```rust
// In main.rs
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("PANIC: {:?}", info);
    // Optional: display on screen
    loop {}
}
```

**Logic Analyzer Pins to Monitor:**
- SPI: SCLK(8), MOSI(10), CS_DISPLAY(21), CS_SD(12)
- Display: DC(4), RST(5), BUSY(6)

### Version Control Strategy
```bash
# Commit after each milestone
git commit -m "Milestone 5: Basic display communication working"

# Tag releases
git tag -a v0.1.0 -m "First working prototype"
```

---

## Resource Library

### Essential Documentation
- **ESP32-C3 Datasheet**: https://www.espressif.com/sites/default/files/documentation/esp32-c3_datasheet_en.pdf
- **ESP32-C3 Technical Reference**: https://www.espressif.com/sites/default/files/documentation/esp32-c3_technical_reference_manual_en.pdf
- **SSD1677 Datasheet**: https://www.good-display.com/companyfile/101.html
- **ESP Rust Book**: https://docs.esp-rs.org/book/
- **Embedded Rust Book**: https://docs.rust-embedded.org/book/

### Crate Documentation
- esp-idf-svc: https://docs.rs/esp-idf-svc/latest/esp_idf_svc/
- embedded-hal: https://docs.rs/embedded-hal/latest/embedded_hal/
- embedded-graphics: https://docs.rs/embedded-graphics/latest/embedded_graphics/
- embedded-sdmmc: https://docs.rs/embedded-sdmmc/latest/embedded_sdmmc/

### Example Code Repositories
- esp-idf-svc examples: https://github.com/esp-rs/esp-idf-svc/tree/master/examples
- ESP32 Rust projects: https://github.com/esp-rs/awesome-esp-rust
- E-ink drivers: https://github.com/caemor/epd-waveshare (Waveshare, but similar concepts)

### Community
- ESP Rust Matrix chat: https://matrix.to/#/#esp-rs:matrix.org
- Embedded Rust Matrix: https://matrix.to/#/#rust-embedded:matrix.org
- r/rust subreddit embedded questions

---

## Timeline Summary

| Phase | Duration | Cumulative | Key Deliverable |
|-------|----------|------------|-----------------|
| 0. Setup | 2 hours | Day 1 | Toolchain working |
| 1. Learning | 8 hours | Day 2-3 | Understand embedded patterns |
| 2. Hardware | 12 hours | Day 4-6 | All peripherals working |
| 3. Display | 20 hours | Day 7-12 | Can render graphics |
| 4. App Logic | 12 hours | Day 13-16 | Can read books |
| 5. Power | 6 hours | Day 17-18 | Deep sleep works |
| 6. Polish | 8 hours | Day 19-21 | User-friendly |
| 7. Testing | 12 hours | Day 22-25 | Production ready |

**Total: 80 hours over 25 days (~3-4 hours/day)**

**Aggressive: 3 weeks full-time (~40 hours/week)**

---

## Success Criteria

By end of project, you should have:
- [ ] Device that boots and shows menu
- [ ] Can browse files on SD card
- [ ] Can read text files with page turning
- [ ] Battery indicator functional
- [ ] Deep sleep reduces power to <100µA
- [ ] Device lasts 2+ weeks on single charge (casual use)
- [ ] Stable (no crashes over 8+ hours)
- [ ] Code is documented and maintainable

---

## Stretch Goals (Post-MVP)

- [ ] EPUB support (requires XML parsing)
- [ ] WiFi for book downloads
- [ ] Dictionary lookup
- [ ] Multiple fonts
- [ ] Reading statistics
- [ ] Bluetooth page-turn remote
- [ ] OTA updates

---

## Getting Help

When stuck:
1. Check datasheet (hardware issues)
2. Read esp-idf-svc docs (API usage)
3. Search GitHub issues in esp-rs repos
4. Ask in Matrix chat (very responsive community)
5. Post minimal reproducible example

Good luck! Start with Phase 0 and work through systematically. The display driver (Phase 3) will be your biggest challenge—budget extra time there.
