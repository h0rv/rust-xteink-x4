//! Minimal, modern UI framework for Xteink X4 e-reader.
//! E-ink optimized: high contrast, no animations, type-safe.

pub mod activity;
pub mod components;
pub(crate) mod helpers;
pub mod theme;

pub use activity::{Activity, ActivityRefreshMode, ActivityResult};
pub use components::{Button, Header, List, Modal, Toast};
pub use theme::{Theme, ThemeMetrics, FONT_CHAR_WIDTH};
