# Glossary of Acronyms & Terms

## Hardware & Platforms

| Term | Full Name | Description |
|------|-----------|-------------|
| **ESP** | **Espressif** | Chinese semiconductor company that makes ESP32/ESP8266 microcontrollers |
| **ESP32** | Espressif System-on-Chip 32-bit | Family of low-cost, low-power microcontrollers with WiFi/Bluetooth |
| **ESP32-C3** | ESP32 RISC-V Core 3 | Specific ESP32 variant using RISC-V architecture (not Xtensa) |
| **MCU** | Microcontroller Unit | Small computer on a single integrated circuit |
| **SoC** | System on Chip | Complete computer system on a single chip (CPU + peripherals) |
| **GPIO** | General Purpose Input/Output | Configurable pins that can be inputs or outputs |
| **ADC** | Analog-to-Digital Converter | Converts analog voltage signals to digital values |
| **SPI** | Serial Peripheral Interface | Synchronous serial communication protocol (fast, 4-wire) |
| **I2C** | Inter-Integrated Circuit | Synchronous serial communication protocol (2-wire, slower than SPI) |
| **UART** | Universal Asynchronous Receiver-Transmitter | Serial communication (like USB-to-serial) |
| **PWM** | Pulse Width Modulation | Technique for controlling power/brightness by rapid on/off switching |
| **CS** | Chip Select | SPI pin that selects which device is active (also called SS) |
| **SS** | Slave Select | Alternative name for CS (Chip Select) |
| **MOSI** | Master Out Slave In | SPI data line from controller to peripheral |
| **MISO** | Master In Slave Out | SPI data line from peripheral to controller |
| **SCLK** | Serial Clock | SPI clock signal line |
| **DC** | Data/Command | E-ink display pin: LOW=command, HIGH=data |
| **RST** | Reset | Hardware reset pin (active low) |
| **PSRAM** | Pseudo-Static RAM | External RAM chip (Xteink X4 doesn't have this) |
| **RISC-V** | Reduced Instruction Set Computer - Five | Open-source CPU architecture (ESP32-C3 uses this) |
| **Xtensa** | - | Proprietary CPU architecture used in older ESP32 models |
| **QFN** | Quad Flat No-leads | IC package type (small, square, no protruding pins) |

## Display Technology

| Term | Full Name | Description |
|------|-----------|-------------|
| **E-ink** | Electronic Ink | Display technology that mimics paper (bistable, low power) |
| **EPD** | Electronic Paper Display | Another name for e-ink displays |
| **LUT** | Look-Up Table | Waveform data that controls how e-ink pixels change states |
| **PPI** | Pixels Per Inch | Display resolution density (Xteink X4 is 220 PPI) |
| **SSD1677** | Solomon Systech Display Driver 1677 | Display controller chip used in GDEQ0426T82 |
| **GDEQ0426T82** | Good Display E-ink Quad 4.26" T82 | Specific e-ink panel model number |

## Storage & Files

| Term | Full Name | Description |
|------|-----------|-------------|
| **SD** | Secure Digital | Memory card standard (microSD in this case) |
| **TF** | TransFlash | Old name for microSD cards (still used in Chinese docs) |
| **FAT32** | File Allocation Table 32-bit | Filesystem format (most SD cards use this) |
| **SPIFFS** | SPI Flash File System | Filesystem for ESP32 internal flash storage |
| **NVS** | Non-Volatile Storage | ESP-IDF key-value storage in flash (survives reboots) |
| **OTA** | Over-The-Air | Wireless firmware updates (WiFi-based) |

## Software & Development

| Term | Full Name | Description |
|------|-----------|-------------|
| **ESP-IDF** | Espressif IoT Development Framework | Official C/C++ framework for ESP32 (includes FreeRTOS) |
| **HAL** | Hardware Abstraction Layer | Code layer that provides generic interface to hardware |
| **PAC** | Peripheral Access Crate | Low-level Rust bindings to MCU registers (auto-generated) |
| **FFI** | Foreign Function Interface | Calling C code from Rust (or vice versa) |
| **RTT** | Real-Time Transfer | Fast debugging protocol (faster than UART/serial) |
| **JTAG** | Joint Test Action Group | Hardware debugging interface (needs special adapter) |
| **ISR** | Interrupt Service Routine | Function that runs when hardware interrupt fires |
| **DMA** | Direct Memory Access | Hardware that moves data without CPU involvement |
| **FreeRTOS** | Free Real-Time Operating System | Embedded OS included in ESP-IDF (task scheduler, etc.) |

## Rust-Specific

| Term | Full Name | Description |
|------|-----------|-------------|
| **no_std** | No Standard Library | Rust code without heap/filesystem/threads (embedded) |
| **embedded-hal** | Embedded Hardware Abstraction Layer | Rust traits for portable embedded code |
| **Cargo** | - | Rust's package manager and build tool |
| **Crate** | - | Rust library/package (like npm packages or Python modules) |
| **defmt** | Deferred Formatting | Efficient logging for embedded (formatting happens on host PC) |
| **probe-rs** | - | Rust tool for flashing and debugging embedded devices |
| **espflash** | - | Tool for flashing ESP32 devices from command line |
| **espup** | ESP Setup | Tool that installs ESP Rust toolchain |

## Power & Battery

| Term | Full Name | Description |
|------|-----------|-------------|
| **mAh** | Milliamp-hours | Battery capacity (Xteink X4 has 650mAh battery) |
| **µA** | Microamps | Current measurement (1/1000 of milliamp) |
| **mA** | Milliamps | Current measurement (1/1000 of amp) |
| **Li-ion** | Lithium-ion | Rechargeable battery chemistry (3.0-4.2V typical) |
| **Deep Sleep** | - | Low-power mode where most of MCU is powered off |

## Communication & Protocols

| Term | Full Name | Description |
|------|-----------|-------------|
| **API** | Application Programming Interface | Set of functions/methods for interacting with software |
| **USB** | Universal Serial Bus | Standard cable/protocol for connecting devices |
| **TTL** | Transistor-Transistor Logic | Voltage levels (0V=LOW, 3.3V/5V=HIGH) |
| **LVDS** | Low-Voltage Differential Signaling | High-speed data transmission (not used in this project) |

## Build & Toolchain

| Term | Full Name | Description |
|------|-----------|-------------|
| **LTO** | Link-Time Optimization | Compiler optimization across all code (smaller binaries) |
| **ELF** | Executable and Linkable Format | Binary file format (before flashing to device) |
| **Linker Script** | - | File that tells linker where to place code/data in memory |
| **Bootloader** | - | First code that runs when device powers on |
| **Partition Table** | - | Defines memory regions (app, data, OTA, etc.) |

## Electrical

| Term | Full Name | Description |
|------|-----------|-------------|
| **PCB** | Printed Circuit Board | The physical board with traces connecting components |
| **SMD/SMT** | Surface-Mount Device/Technology | Components soldered directly to PCB surface (no through-holes) |
| **Pull-up/Pull-down** | - | Resistor that sets default HIGH/LOW state on input pins |
| **Voltage Divider** | - | Two resistors that reduce voltage (used for battery monitoring) |
| **Attenuation** | - | Signal reduction (ADC uses this for measuring higher voltages) |
| **dB** | Decibels | Logarithmic unit (ADC attenuation: DB_12 = 12dB = ~3.3V max) |

## File Formats & Standards

| Term | Full Name | Description |
|------|-----------|-------------|
| **EPUB** | Electronic Publication | Standard ebook format (ZIP archive with HTML/CSS) |
| **TOML** | Tom's Obvious, Minimal Language | Config file format used by Cargo |
| **JSON** | JavaScript Object Notation | Data interchange format |
| **XML** | Extensible Markup Language | Markup language (used inside EPUB files) |
| **UTF-8** | Unicode Transformation Format 8-bit | Character encoding (supports all languages) |

## Development Tools

| Term | Full Name | Description |
|------|-----------|-------------|
| **IDE** | Integrated Development Environment | Code editor with build/debug tools |
| **CLI** | Command-Line Interface | Text-based program interface (opposite of GUI) |
| **GUI** | Graphical User Interface | Visual interface with windows/buttons |
| **REPL** | Read-Eval-Print Loop | Interactive programming shell |
| **Flamegraph** | - | Visualization of where CPU time is spent (profiling) |
| **Logic Analyzer** | - | Hardware tool that captures/displays digital signals |
| **Multimeter** | - | Tool for measuring voltage, current, resistance |

## Memory & Storage

| Term | Full Name | Description |
|------|-----------|-------------|
| **RAM** | Random Access Memory | Volatile memory (lost on power off) - ESP32-C3 has 400KB |
| **ROM** | Read-Only Memory | Permanent memory (bootloader, system code) |
| **Flash** | Flash Memory | Non-volatile storage (16MB on Xteink X4) |
| **SRAM** | Static RAM | Fast memory that doesn't need refreshing |
| **Stack** | - | Memory region for function calls and local variables |
| **Heap** | - | Memory region for dynamic allocations (malloc/new) |
| **Buffer** | - | Temporary memory area for data (e.g., display framebuffer) |
| **Framebuffer** | - | Memory holding pixel data before sending to display |

## Misc Abbreviations

| Term | Full Name | Description |
|------|-----------|-------------|
| **MVP** | Minimum Viable Product | Simplest version that works |
| **PoC** | Proof of Concept | Quick prototype to test feasibility |
| **RTFM** | Read The Fine Manual | Polite reminder to check documentation |
| **PEBKAC** | Problem Exists Between Keyboard And Chair | User error (use sparingly!) |
| **YMMV** | Your Mileage May Vary | Results may differ in your situation |
| **TL;DR** | Too Long; Didn't Read | Summary of long content |

## Project-Specific (Xteink X4)

| Term | Meaning |
|------|---------|
| **Xteink** | Brand name (Chinese e-reader manufacturer) |
| **X4** | Model number (4.26" display) |
| **GDEQ0426T82** | Display panel part number from Good Display |
| **GxEPD2** | Arduino library for e-ink displays (used in original C++ code) |

---

## Quick Reference: Pin Names

For ESP32-C3 developers coming from Arduino:

| ESP32 Name | Arduino Equivalent | Purpose |
|------------|-------------------|---------|
| GPIO0-21 | D0-D21 | Digital pins |
| ADC1_CH0 | A0 | Analog input channel 0 |
| MOSI | MOSI | SPI Master Out |
| MISO | MISO | SPI Master In |
| SCLK | SCK | SPI Clock |
| SDA | SDA | I2C Data |
| SCL | SCL | I2C Clock |
| TX | TX | UART Transmit |
| RX | RX | UART Receive |

**Note:** ESP32-C3 is 3.3V logic only (unlike 5V Arduino Uno). Never connect 5V signals directly to GPIO pins!

---

## Units Quick Reference

| Unit | Symbol | Equivalent |
|------|--------|------------|
| Kilobyte | KB | 1,024 bytes |
| Megabyte | MB | 1,024 KB = 1,048,576 bytes |
| Kilohertz | kHz | 1,000 Hz (cycles per second) |
| Megahertz | MHz | 1,000 kHz = 1,000,000 Hz |
| Millivolt | mV | 1/1,000 volt |
| Milliamp | mA | 1/1,000 amp |
| Microamp | µA | 1/1,000,000 amp |
| Millisecond | ms | 1/1,000 second |
| Microsecond | µs | 1/1,000,000 second |
