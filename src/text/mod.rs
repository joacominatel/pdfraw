//! Layout reconstruction and word extraction.
//!
//! - [`cluster`] — generic 1D greedy clustering.
//! - [`extractor`] — Char→Word→Line→TextMap pipeline.
//! - [`options`] — [`options::TextOptions`] builder.
//! - [`textmap`] — output-to-source mapping.
//! - [`ligatures`] — Unicode ligature expansion.

pub mod cluster;
pub mod dedupe;
pub mod extractor;
pub mod ligatures;
pub mod options;
pub mod textmap;
