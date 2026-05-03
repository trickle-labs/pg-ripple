//! Fuzzy SPARQL guard functions — `pg:fuzzy_match()` / `pg:token_set_ratio()`.
//!
//! The actual `#[pg_extern]` functions (`_fuzzy_match_guard`, `_token_set_ratio_guard`)
//! and their shared pre-flight check (`fuzzy_guard_checks`) live in the parent
//! `mod.rs` module (`src/uncertain_knowledge_api/mod.rs`).
//!
//! This sub-module is reserved for future expansion of the fuzzy SPARQL engine
//! when the trigram + token-set family grows beyond the current two entry points.
