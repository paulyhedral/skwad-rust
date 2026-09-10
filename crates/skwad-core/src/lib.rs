//! Shared types, error model, constants, and localization lookup for the
//! Skwad Rust port.

pub mod consts;
pub mod error;
pub mod l10n;

pub use error::{Error, Result};
pub use l10n::t;
