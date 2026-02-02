//! Hardware interface abstraction
//!
//! This module provides the [`DisplayInterface`] trait and the [`Interface`] struct
//! for communicating with the SSD1677 controller over SPI.
//!
//! ## Hardware Requirements
//!
//! The SSD1677 requires:
//! - SPI bus (MOSI + SCK)
//! - 3 GPIO pins:
//!   - **DC**: Data/Command select (output)
//!   - **RST**: Reset (output, active low)
//!   - **BUSY**: Busy status (input, active high)
//!
//! ## Example
//!
//! ```rust,ignore
//! use ssd1677::Interface;
//! use embedded_hal::spi::SpiDevice;
//! use embedded_hal::digital::{InputPin, OutputPin};
//!
//! // Create interface with SPI and GPIO pins
//! let interface = Interface::new(spi_device, dc_pin, rst_pin, busy_pin);
//!
//! // Send command
//! interface.send_command(0x12)?; // Soft reset
//!
//! // Send data
//! interface.send_data(&[0xFF, 0x00, 0xFF])?;
//!
//! // Wait for display ready
//! interface.busy_wait(&mut delay)?;
//! ```

use core::fmt::Debug;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

/// Trait for hardware interface to SSD1677 controller
///
/// This trait abstracts over different hardware implementations,
/// allowing the [`Display`](crate::display::Display) to work with any
/// SPI + GPIO implementation that satisfies embedded-hal traits.
///
/// ## Implementing
///
/// For most cases, use the provided [`Interface`] struct. If you need
/// custom behavior (e.g., different pin polarities, additional CS control),
/// implement this trait on your own type.
pub trait DisplayInterface {
    /// Error type for interface operations
    ///
    /// Must implement [`Debug`] for error reporting.
    type Error: Debug;

    /// Send a command byte to the controller
    ///
    /// The implementation must:
    /// 1. Set DC pin low (command mode)
    /// 2. Send the command byte over SPI
    ///
    /// # Errors
    ///
    /// Returns an error if SPI communication or GPIO fails.
    fn send_command(&mut self, command: u8) -> Result<(), Self::Error>;

    /// Send data bytes to the controller
    ///
    /// The implementation must:
    /// 1. Set DC pin high (data mode)
    /// 2. Send the data bytes over SPI
    ///
    /// # Arguments
    ///
    /// * `data` - Slice of bytes to send
    ///
    /// # Errors
    ///
    /// Returns an error if SPI communication or GPIO fails.
    fn send_data(&mut self, data: &[u8]) -> Result<(), Self::Error>;

    /// Perform hardware reset
    ///
    /// The implementation must:
    /// 1. Set RST pin low
    /// 2. Wait at least 10ms
    /// 3. Set RST pin high
    /// 4. Wait at least 10ms
    ///
    /// # Arguments
    ///
    /// * `delay` - Delay implementation for timing
    fn reset<D: DelayNs>(&mut self, delay: &mut D);

    /// Wait for busy pin to go low (with timeout)
    ///
    /// Polls the BUSY pin until it goes low (display ready) or timeout occurs.
    /// BUSY is active high - when high, the display is processing a command.
    ///
    /// # Arguments
    ///
    /// * `delay` - Delay implementation for polling interval
    ///
    /// # Errors
    ///
    /// Returns [`InterfaceError::Timeout`] if BUSY doesn't go low within
    /// the implementation-specific timeout period.
    fn busy_wait<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Self::Error>;
}

/// Errors that can occur at the interface level
///
/// Generic over SPI and GPIO error types.
#[derive(Debug)]
pub enum InterfaceError<SpiErr, PinErr> {
    /// SPI communication error
    Spi(SpiErr),
    /// GPIO pin error
    Pin(PinErr),
    /// Timeout waiting for busy pin
    Timeout,
}

impl<SpiErr: Debug, PinErr: Debug> core::fmt::Display for InterfaceError<SpiErr, PinErr> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            InterfaceError::Spi(e) => write!(f, "SPI error: {e:?}"),
            InterfaceError::Pin(e) => write!(f, "Pin error: {e:?}"),
            InterfaceError::Timeout => write!(f, "Timeout waiting for display"),
        }
    }
}

impl<SpiErr: Debug, PinErr: Debug> core::error::Error for InterfaceError<SpiErr, PinErr> {}

/// Hardware interface implementation for SSD1677
///
/// Implements [`DisplayInterface`] for embedded-hal v1.0 SPI and GPIO traits.
///
/// ## Type Parameters
///
/// * `SPI` - SPI device implementing [`SpiDevice`]
/// * `DC` - Data/Command pin implementing [`OutputPin`]
/// * `RST` - Reset pin implementing [`OutputPin`]
/// * `BUSY` - Busy pin implementing [`InputPin`]
///
/// ## Example
///
/// ```rust,ignore
/// use ssd1677::Interface;
///
/// let interface = Interface::new(
///     spi_device,  // SpiDevice
///     dc_pin,      // OutputPin
///     rst_pin,     // OutputPin
///     busy_pin,    // InputPin
/// );
///
/// // Use with Display
/// let display = Display::new(interface, config);
/// ```
pub struct Interface<SPI, DC, RST, BUSY> {
    /// SPI device for communication
    spi: SPI,
    /// Data/Command select pin (low=command, high=data)
    dc: DC,
    /// Reset pin (active low)
    rst: RST,
    /// Busy pin (active high)
    busy: BUSY,
}

impl<SPI, DC, RST, BUSY> Interface<SPI, DC, RST, BUSY>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
{
    /// Create a new Interface
    ///
    /// # Arguments
    ///
    /// * `spi` - SPI device (must implement [`SpiDevice`])
    /// * `dc` - Data/Command pin (output, low=command, high=data)
    /// * `rst` - Reset pin (output, active low)
    /// * `busy` - Busy pin (input, active high)
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// use ssd1677::Interface;
    ///
    /// let interface = Interface::new(spi, dc, rst, busy);
    /// ```
    pub fn new(spi: SPI, dc: DC, rst: RST, busy: BUSY) -> Self {
        Self { spi, dc, rst, busy }
    }
}

impl<SPI, DC, RST, BUSY, PinErr> DisplayInterface for Interface<SPI, DC, RST, BUSY>
where
    SPI: SpiDevice,
    SPI::Error: Debug,
    DC: OutputPin<Error = PinErr>,
    RST: OutputPin<Error = PinErr>,
    BUSY: InputPin<Error = PinErr>,
    PinErr: Debug,
{
    type Error = InterfaceError<SPI::Error, PinErr>;

    fn send_command(&mut self, command: u8) -> Result<(), Self::Error> {
        self.dc.set_low().map_err(|e| InterfaceError::Pin(e))?;
        self.spi
            .write(&[command])
            .map_err(|e| InterfaceError::Spi(e))?;
        Ok(())
    }

    fn send_data(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.dc.set_high().map_err(|e| InterfaceError::Pin(e))?;
        self.spi.write(data).map_err(|e| InterfaceError::Spi(e))?;
        Ok(())
    }

    fn reset<D: DelayNs>(&mut self, delay: &mut D) {
        // Reset sequence: LOW -> wait 10ms -> HIGH -> wait 10ms
        let _ = self.rst.set_low();
        delay.delay_ms(10);
        let _ = self.rst.set_high();
        delay.delay_ms(10);
    }

    fn busy_wait<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Self::Error> {
        let mut iterations = 0u32;
        const TIMEOUT_MS: u32 = 30_000; // 30 second timeout

        loop {
            match self.busy.is_high() {
                Ok(true) => {
                    delay.delay_ms(1);
                    iterations += 1;
                    if iterations >= TIMEOUT_MS {
                        return Err(InterfaceError::Timeout);
                    }
                }
                Ok(false) => return Ok(()),
                Err(e) => return Err(InterfaceError::Pin(e)),
            }
        }
    }
}
