-- pg_ripple upgrade: 0.125.0 → 0.126.0
-- v0.126.0 FEAT-03: Per-endpoint federation credentials (OAuth2 Bearer / API-key)
--
-- This script adds the _pg_ripple.federation_credentials table.
-- Tokens are stored encrypted via pgcrypto pgp_sym_encrypt using the
-- pg_ripple.federation_credential_key GUC (never visible via SHOW).
--
-- New SQL functions added by this release:
--   pg_ripple.set_federation_credential(endpoint_iri, auth_type, token)
--   pg_ripple.rotate_federation_credential(endpoint_iri, new_token)
--   pg_ripple.federation_credential_audit()
--
-- HTTP endpoint:
--   GET /federation/{endpoint}/auth-status  (write-auth required)
--
-- SSRF guard validates endpoint URI BEFORE credential lookup to prevent
-- credential-oracle attacks.

-- ── Schema change: add federation_credentials table ──────────────────────────

CREATE TABLE IF NOT EXISTS _pg_ripple.federation_credentials (
    endpoint_iri    TEXT        NOT NULL PRIMARY KEY
                    REFERENCES _pg_ripple.federation_endpoints(url)
                    ON DELETE CASCADE ON UPDATE CASCADE,
    auth_type       TEXT        NOT NULL
                    CHECK (auth_type IN ('bearer', 'apikey', 'none')),
    encrypted_token BYTEA       NOT NULL DEFAULT ''::bytea,
    header_name     TEXT        NOT NULL DEFAULT 'Authorization',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    rotated_at      TIMESTAMPTZ,
    last_used_at    TIMESTAMPTZ
);

COMMENT ON TABLE _pg_ripple.federation_credentials IS
    'Per-endpoint federation credentials (v0.126.0 FEAT-03). '
    'Tokens encrypted with pgp_sym_encrypt; decrypted in-process at query time. '
    'Use pg_ripple.set_federation_credential() and '
    'pg_ripple.rotate_federation_credential() to manage credentials. '
    'Never query encrypted_token directly — use federation_credential_audit() '
    'for operational metadata.';

-- ── Update extension version ──────────────────────────────────────────────────
-- (pgrx handles this automatically via pg_extension_config_dump)
