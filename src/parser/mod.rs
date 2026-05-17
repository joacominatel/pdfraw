//! PDF parsing backends.
//!
//! The MVP only ships [`lopdf_backend`]; the [`Backend`] trait is reserved
//! for future alternative backends (e.g. an FFI-based one).

pub mod lopdf_backend;
pub mod scan_detect;
