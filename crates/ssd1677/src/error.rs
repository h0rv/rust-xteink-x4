//! Error types for the driver
//!
//! This module defines error types for configuration building ([`BuilderError`])
//! and display operations ([`Error`]).
//!
//! ## Error Types
//!
//! - [`BuilderError`] - Errors during configuration construction
//! - [`Error`] - Runtime errors during display operations
//! - [`InterfaceError`](crate::interface::InterfaceError) - Low-level hardware communication errors
//!
//! ## Example
//!
//! ```
//! use ssd1677::{Builder, Dimensions, BuilderError};
//!
//! // Missing dimensions
//! let result = Builder::new().build();
//! assert!(matches!(result, Err(BuilderError::MissingDimensions)));
//!
//! // Invalid dimensions
//! let result = Dimensions::new(1000, 500); // Too large
//! assert!(result.is_err());
//! ```

use crate::interface::DisplayInterface;

/// Maximum gate outputs (rows) supported by SSD1677 controller
///
/// The SSD1677 supports up to 800 gate driver outputs.
pub const MAX_GATE_OUTPUTS: u16 = 800;

/// Maximum source outputs (columns) supported by SSD1677 controller
///
/// The SSD1677 supports up to 480 source driver outputs.
pub const MAX_SOURCE_OUTPUTS: u16 = 480;

/// Errors that can occur when interacting with the display
///
/// Generic over the interface type to preserve the specific error type.
/// This allows error handling code to match on the underlying hardware error.
#[derive(Debug)]
pub enum Error<I: DisplayInterface> {
    /// Interface error (SPI/GPIO)
    ///
    /// Wraps the underlying hardware error from the [`DisplayInterface`] implementation.
    Interface(I::Error),
    /// Invalid dimensions provided
    ///
    /// Dimensions must satisfy:
    /// - 1 <= rows <= MAX_GATE_OUTPUTS (800)
    /// - 8 <= cols <= MAX_SOURCE_OUTPUTS (480)
    /// - cols must be a multiple of 8
    InvalidDimensions {
        /// Number of rows (height) requested
        rows: u16,
        /// Number of columns (width) requested
        cols: u16,
    },
    /// Invalid rotation value
    ///
    /// Currently unused as rotation is type-safe via [`Rotation`](crate::config::Rotation) enum.
    InvalidRotation,
    /// Buffer is too small for the display
    ///
    /// The provided buffer must be at least `dimensions.buffer_size()` bytes.
    BufferTooSmall {
        /// Required buffer size in bytes
        required: usize,
        /// Provided buffer size in bytes
        provided: usize,
    },
}

impl<I: DisplayInterface> core::fmt::Display for Error<I> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Interface(_) => write!(f, "Interface error"),
            Error::InvalidDimensions { rows, cols } => {
                write!(f, "Invalid dimensions: {rows}x{cols}")
            }
            Error::InvalidRotation => write!(f, "Invalid rotation"),
            Error::BufferTooSmall { required, provided } => {
                write!(
                    f,
                    "Buffer too small: required {required} bytes, provided {provided}"
                )
            }
        }
    }
}

impl<I: DisplayInterface + core::fmt::Debug> core::error::Error for Error<I> {}

/// Errors that can occur when building configuration
///
/// These errors occur during the builder pattern before the display is created.
#[derive(Debug)]
pub enum BuilderError {
    /// Dimensions were not specified
    ///
    /// [`Builder::dimensions()`](crate::config::Builder::dimensions) must be called before building.
    MissingDimensions,
    /// Invalid dimensions provided
    ///
    /// See [`Dimensions::new()`](crate::config::Dimensions::new) for constraints.
    InvalidDimensions {
        /// Number of rows (height) requested
        rows: u16,
        /// Number of columns (width) requested
        cols: u16,
    },
}

impl core::fmt::Display for BuilderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BuilderError::MissingDimensions => write!(f, "Dimensions must be specified"),
            BuilderError::InvalidDimensions { rows, cols } => write!(
                f,
                "Invalid dimensions {rows}x{cols} (max {MAX_GATE_OUTPUTS}x{MAX_SOURCE_OUTPUTS}, cols must be multiple of 8)"
            ),
        }
    }
}

impl core::error::Error for BuilderError {}
