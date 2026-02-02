//! SSD1677 E-Paper Display Driver
//!
//! A driver for the SSD1677 e-paper display controller supporting displays up to 800x480 pixels.
//!
//! ## Features
//!
//! - `no_std` compatible
//! - `embedded-hal` v1.0 support
//! - `embedded-graphics` integration (with `graphics` feature)
//! - Configurable display dimensions
//! - Full and fast refresh modes
//! - Custom LUT support
//! - Rotation support
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ssd1677::{Builder, Dimensions, Display, Interface, Rotation};
//!
//! let interface = Interface::new(spi, dc, rst, busy);
//! let config = Builder::new()
//!     .dimensions(Dimensions::new(480, 800)?)
//!     .rotation(Rotation::Rotate0)
//!     .build()?;
//!
//! let mut display = Display::new(interface, config);
//! display.reset(&mut delay)?;
//! ```

#![no_std]
#![deny(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
#![allow(clippy::if_not_else)]
#![allow(clippy::redundant_pub_crate)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::derive_partial_eq_without_eq)]
#![allow(clippy::cargo_common_metadata)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::use_self)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::incompatible_msrv)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_sign_loss)]

extern crate alloc;

/// Color types for tri-color e-paper displays
pub mod color;
/// SSD1677 command definitions
pub mod command;
/// Display configuration types and builder
pub mod config;
/// Core display operations
pub mod display;
/// Error types for the driver
pub mod error;
/// Hardware interface abstraction
pub mod interface;
/// Coordinate rotation utilities
pub mod rotation;

/// Graphics support via embedded-graphics (requires `graphics` feature)
#[cfg(feature = "graphics")]
pub mod graphics;

pub use color::Color;
pub use config::{Builder, Config, Dimensions, MAX_GATE_OUTPUTS, MAX_SOURCE_OUTPUTS, Rotation};
pub use display::Display;
pub use error::{BuilderError, Error};
pub use interface::InterfaceError;
pub use interface::{DisplayInterface, Interface};

#[cfg(feature = "graphics")]
pub use graphics::GraphicDisplay;
