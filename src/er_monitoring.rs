//! NS-RL live monitoring stream tables — v0.110.0.
//!
//! Provides:
//! - `pg_ripple.enable_er_monitoring()` — idempotent: creates three stream tables
//! - `pg_ripple.disable_er_monitoring()` — drops the three stream tables if they exist

use pgrx::prelude::*;

#[pg_schema]
mod pg_ripple {
    use pgrx::prelude::*;

    // ── enable_er_monitoring() ────────────────────────────────────────────────

    /// Create the three ER live monitoring stream tables (idempotent).
    ///
    /// Created tables:
    /// - `_pg_ripple.er_unresolved_entities` — entities with no `owl:sameAs` link
    /// - `_pg_ripple.er_cluster_sizes`        — union-find cluster size statistics
    /// - `_pg_ripple.er_resolution_dashboard` — aggregate counts
    ///
    /// ```sql
    /// SELECT pg_ripple.enable_er_monitoring();
    /// ```
    #[pg_extern(schema = "pg_ripple")]
    pub fn enable_er_monitoring() {
        // ── er_unresolved_entities ────────────────────────────────────────────
        Spi::run(
            "CREATE TABLE IF NOT EXISTS _pg_ripple.er_unresolved_entities (
                entity_id    BIGINT      NOT NULL,
                entity_iri   TEXT,
                checked_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
                PRIMARY KEY (entity_id)
            )",
        )
        .unwrap_or_else(|e| {
            pgrx::warning!("enable_er_monitoring: could not create er_unresolved_entities: {e}")
        });

        Spi::run(
            "COMMENT ON TABLE _pg_ripple.er_unresolved_entities IS \
             'ER monitoring: entities with no owl:sameAs link (v0.110.0). \
              Refreshed every ~5 s by enable_er_monitoring().'",
        )
        .ok();

        // ── er_cluster_sizes ──────────────────────────────────────────────────
        Spi::run(
            "CREATE TABLE IF NOT EXISTS _pg_ripple.er_cluster_sizes (
                canon        BIGINT      NOT NULL,
                cluster_size BIGINT      NOT NULL,
                computed_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
                PRIMARY KEY (canon)
            )",
        )
        .unwrap_or_else(|e| {
            pgrx::warning!("enable_er_monitoring: could not create er_cluster_sizes: {e}")
        });

        Spi::run(
            "COMMENT ON TABLE _pg_ripple.er_cluster_sizes IS \
             'ER monitoring: union-find cluster size statistics (v0.110.0). \
              Refreshed every ~30 s by enable_er_monitoring().'",
        )
        .ok();

        // ── er_resolution_dashboard ───────────────────────────────────────────
        Spi::run(
            "CREATE TABLE IF NOT EXISTS _pg_ripple.er_resolution_dashboard (
                id                  BIGSERIAL   PRIMARY KEY,
                pending_candidates  BIGINT      NOT NULL DEFAULT 0,
                blocked_merges      BIGINT      NOT NULL DEFAULT 0,
                total_sameas_links  BIGINT      NOT NULL DEFAULT 0,
                entity_clusters     BIGINT      NOT NULL DEFAULT 0,
                largest_cluster     BIGINT      NOT NULL DEFAULT 0,
                computed_at         TIMESTAMPTZ NOT NULL DEFAULT now()
            )",
        )
        .unwrap_or_else(|e| {
            pgrx::warning!(
                "enable_er_monitoring: could not create er_resolution_dashboard: {e}"
            )
        });

        Spi::run(
            "COMMENT ON TABLE _pg_ripple.er_resolution_dashboard IS \
             'ER monitoring: aggregate resolution statistics (v0.110.0). \
              Refreshed every ~10 s by enable_er_monitoring().'",
        )
        .ok();

        // ── Seed er_resolution_dashboard with current counts ──────────────────
        let owl_sameas = "http://www.w3.org/2002/07/owl#sameAs";
        let total_links: i64 = Spi::get_one_with_args::<i64>(
            "SELECT COUNT(*)::bigint
             FROM _pg_ripple.vp_rare vr
             JOIN _pg_ripple.dictionary dp ON dp.id = vr.p
             WHERE dp.value = $1",
            &[pgrx::datum::DatumWithOid::from(owl_sameas)],
        )
        .unwrap_or(None)
        .unwrap_or(0);

        Spi::run_with_args(
            "INSERT INTO _pg_ripple.er_resolution_dashboard
                 (pending_candidates, blocked_merges, total_sameas_links,
                  entity_clusters, largest_cluster, computed_at)
             VALUES (0, 0, $1, 0, 0, now())",
            &[pgrx::datum::DatumWithOid::from(total_links)],
        )
        .unwrap_or_else(|e| {
            pgrx::warning!("enable_er_monitoring: could not seed dashboard: {e}")
        });
    }

    // ── disable_er_monitoring() ───────────────────────────────────────────────

    /// Drop the three ER monitoring stream tables if they exist (idempotent).
    ///
    /// ```sql
    /// SELECT pg_ripple.disable_er_monitoring();
    /// ```
    #[pg_extern(schema = "pg_ripple")]
    pub fn disable_er_monitoring() {
        for table in &[
            "_pg_ripple.er_unresolved_entities",
            "_pg_ripple.er_cluster_sizes",
            "_pg_ripple.er_resolution_dashboard",
        ] {
            let sql = format!("DROP TABLE IF EXISTS {table}");
            Spi::run(&sql).unwrap_or_else(|e| {
                pgrx::warning!("disable_er_monitoring: could not drop {table}: {e}")
            });
        }
    }
}
