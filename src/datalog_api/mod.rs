//! pg_ripple SQL API — Datalog Reasoning Engine (v0.10.0+).
//!
//! # Module layout (v0.114.0)
//!
//! | Sub-module  | Contents |
//! |---|---|
//! | `parse`     | Rule loading and management (`load_rules`, `list_rules`, `add_rule`, …) |
//! | `validate`  | Inference execution (`infer`, `infer_with_stats`, `infer_goal`, `infer_wfs`, …) |
//! | `explain`   | Explain / audit (`explain_datalog`, `explain_inference`, `justify`, …) |
//! | `conflict`  | Lattice, tabling, constraint checking, hypothetical inference, `rule_conflicts` |

pub mod conflict;
pub mod explain;
pub mod parse;
pub mod validate;
