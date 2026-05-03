//! SHACL soft-scoring helpers — `shacl_score()`, `shacl_report_scored()`, `log_shacl_score()`.
//!
//! The actual `#[pg_extern]` SQL API functions live in the parent
//! `mod.rs` (`src/uncertain_knowledge_api/mod.rs`) and delegate to
//! `crate::shacl_scoring`.
//!
//! This sub-module is reserved for future extraction of the scored-SHACL
//! report formatting and score-log management helpers.
