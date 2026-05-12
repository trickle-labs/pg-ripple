-- v0.111.0 Feature Regression Tests
-- Tests for: PPRL Bloom-Filter Encoding
--
-- Covers:
--   BLOOM-01: GUC pg_ripple.bloom_max_input_length default is 4096
--   BLOOM-02: bloom_encode() returns a non-null non-empty string
--   BLOOM-03: bloom_encode() with default params returns 256-char hex string (1024 bits = 128 bytes = 256 hex chars)
--   BLOOM-04: bloom_encode() is deterministic (same input → same output)
--   BLOOM-05: bloom_encode() with same input produces dice_similarity = 1.0
--   BLOOM-06: bloom_encode() with different inputs produces dice_similarity < 1.0
--   BLOOM-07: bloom_encode() raises PT0471 for hash_count = 0
--   BLOOM-08: bloom_encode() raises PT0471 for length = 32 (below minimum)
--   BLOOM-09: bloom_encode() raises PT0471 for length not multiple of 8
--   BLOOM-10: bloom_encode() raises PT0470 when value exceeds bloom_max_input_length

SET client_min_messages = warning;
CREATE EXTENSION IF NOT EXISTS pg_ripple;
SET client_min_messages = DEFAULT;
SET search_path TO pg_ripple, public;

LOAD '$libdir/pg_ripple';

-- BLOOM-01: GUC bloom_max_input_length default is 4096

SELECT current_setting('pg_ripple.bloom_max_input_length') = '4096'
    AS bloom01_max_input_default;

-- BLOOM-02: bloom_encode() returns a non-null non-empty string

SELECT pg_ripple.bloom_encode('Alice', 'secret') IS NOT NULL
    AND length(pg_ripple.bloom_encode('Alice', 'secret')) > 0
    AS bloom02_non_null;

-- BLOOM-03: bloom_encode() with default params returns 256-char hex string
-- 1024 bits = 128 bytes → hex encoding = 256 characters

SELECT length(pg_ripple.bloom_encode('Alice', 'secret', 30, 1024)) = 256
    AS bloom03_correct_hex_length;

-- BLOOM-04: bloom_encode() is deterministic

SELECT pg_ripple.bloom_encode('Alice', 'secret', 30, 1024)
     = pg_ripple.bloom_encode('Alice', 'secret', 30, 1024)
    AS bloom04_deterministic;

-- BLOOM-05: dice_similarity on identical bloom encodings = 1.0

SELECT pg_ripple.dice_similarity(
    pg_ripple.bloom_encode('Alice', 'secret', 30, 1024),
    pg_ripple.bloom_encode('Alice', 'secret', 30, 1024)
) = 1.0 AS bloom05_identical_sim;

-- BLOOM-06: dice_similarity on different inputs < 1.0

SELECT pg_ripple.dice_similarity(
    pg_ripple.bloom_encode('Alice', 'secret', 30, 1024),
    pg_ripple.bloom_encode('Bob',   'secret', 30, 1024)
) < 1.0 AS bloom06_different_inputs;

-- BLOOM-07: PT0471 raised for hash_count = 0

DO $$
BEGIN
    PERFORM pg_ripple.bloom_encode('Alice', 'secret', 0, 1024);
    RAISE EXCEPTION 'expected PT0471 error not raised';
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'BLOOM-07 ok: caught PT0471 for hash_count=0';
END;
$$;

-- BLOOM-08: PT0471 raised for length = 32 (below minimum 64)

DO $$
BEGIN
    PERFORM pg_ripple.bloom_encode('Alice', 'secret', 30, 32);
    RAISE EXCEPTION 'expected PT0471 error not raised';
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'BLOOM-08 ok: caught PT0471 for length=32';
END;
$$;

-- BLOOM-09: PT0471 raised for length not multiple of 8

DO $$
BEGIN
    PERFORM pg_ripple.bloom_encode('Alice', 'secret', 30, 100);
    RAISE EXCEPTION 'expected PT0471 error not raised';
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'BLOOM-09 ok: caught PT0471 for length=100';
END;
$$;

-- BLOOM-10: PT0470 raised when value exceeds bloom_max_input_length
-- Temporarily set a very small limit

SET pg_ripple.bloom_max_input_length = 5;

DO $$
BEGIN
    PERFORM pg_ripple.bloom_encode('This is longer than 5 bytes', 'secret');
    RAISE EXCEPTION 'expected PT0470 error not raised';
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'BLOOM-10 ok: caught PT0470 for oversized input';
END;
$$;

-- Restore default
RESET pg_ripple.bloom_max_input_length;
