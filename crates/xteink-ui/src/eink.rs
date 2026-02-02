//! E-ink display driver wrapper for Xteink X4
//!
//! Provides a simplified interface to the SSD1677 display driver.
//! This keeps the firmware decoupled from specific driver implementations.

/// E-ink display handle
///
/// This is a type alias for the underlying driver display.
/// The firmware uses this type but doesn't need to know about ssd1677 specifics.
pub type EinkDisplay<Interface> = ssd1677::Display<Interface>;

/// E-ink interface handle  
pub type EinkInterface<SPI, DC, RST, BUSY> = ssd1677::Interface<SPI, DC, RST, BUSY>;

/// Display refresh modes
pub use ssd1677::RefreshMode;
/// Partial update region helpers
pub use ssd1677::{Region, UpdateRegion};

/// Display configuration builder
pub use ssd1677::Builder;

/// Display dimensions
pub use ssd1677::Dimensions;

/// Display interface trait
pub use ssd1677::DisplayInterface;
/// RAM X addressing unit (pixels or bytes)
pub use ssd1677::RamXAddressing;
/// Display rotation
pub use ssd1677::Rotation;

/// Re-export ssd1677 errors
pub use ssd1677::Error as EinkError;

/// Re-export the driver crate (only for advanced usage)
pub use ssd1677;
