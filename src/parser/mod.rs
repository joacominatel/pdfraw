//! PDF parsing backends.
//!
//! The MVP only ships [`lopdf_backend`]; future alternative backends
//! (e.g. an FFI-based one) would live alongside it.

pub(crate) mod fonts;
pub mod lopdf_backend;
pub mod scan_detect;
