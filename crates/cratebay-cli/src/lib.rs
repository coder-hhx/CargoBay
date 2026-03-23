//! CrateBay CLI library.
//!
//! The desktop app is the primary UX, but keeping the command implementations in a
//! library makes it possible to unit test and reuse them from the `cratebay` binary.

pub mod commands;
