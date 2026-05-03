//! Confidence table helpers — referenced from the main `uncertain_knowledge_api` module.
//!
//! The actual `load_triples_with_confidence()` and `vacuum_confidence()` SQL API
//! is exposed via `#[pg_extern]` functions in the parent `mod.rs` and calls
//! `crate::bulk_load::load_triples_with_confidence` and inline SPI respectively.
//!
//! This sub-module is reserved for future extraction of the confidence-table
//! management helpers when they grow beyond the current inline implementation.
