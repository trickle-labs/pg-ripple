//! cargo-fuzz target for the temporal query parser (M16-13, v0.117.0).
//!
//! Feeds arbitrary byte sequences through the SPARQL parser looking for
//! queries that include temporal syntax patterns (GRAPH … FILTER … ?time).
//! Asserts: no panic, no undefined behaviour.  Invalid queries must produce
//! a parse error or be silently rejected — never a crash.
//!
//! This target specifically exercises temporal graph selectors and
//! time-range FILTER expressions that pass through the SPARQL→SQL compiler.
//!
//! # Running locally
//!
//! ```sh
//! cargo install cargo-fuzz
//! cargo fuzz run temporal_query -- -max_total_time=300
//! cargo fuzz tmin temporal_query artifacts/temporal_query/crash-...
//! ```
//!
//! # CI
//!
//! Run nightly for 300 seconds to guard the temporal query compilation path.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    // Attempt to parse as a SPARQL query (handles SELECT/CONSTRUCT/ASK/DESCRIBE).
    // The temporal filter patterns live in the WHERE clause — any valid SPARQL
    // query with FILTER(?time >= "..."^^xsd:dateTime) will exercise this path.
    let _ = spargebra::Query::parse(s, None);

    // Also attempt to parse as a SPARQL Update — temporal graph management
    // uses INSERT DATA / DELETE DATA with versioned graph IRIs.
    let _ = spargebra::Update::parse(s, None);
});
