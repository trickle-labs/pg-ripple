# SQL Migration Scripts
This document lists every migration script in sequential order from the initial
installation (`0.1.0`) to the current version (`0.116.0`).

## Install from scratch

```sql
CREATE EXTENSION pg_ripple;  -- installs the default_version from pg_ripple.control
```

## Upgrade an existing installation

PostgreSQL automatically applies the chain of migration scripts when you run:

```sql
ALTER EXTENSION pg_ripple UPDATE;
```

PostgreSQL walks the chain from your current version to `default_version` in
`pg_ripple.control`, applying each `pg_ripple--<from>--<to>.sql` script in turn.
No manual script execution is required.

## Checksum verification

To verify the integrity of the migration scripts on your system:

```bash
# From the extension source directory:
sha256sum sql/pg_ripple--*.sql

# Compare against the SBOM (pg_ripple.cdx.json) for the installed version:
jq '.components[] | select(.name == "pg_ripple-sql") | .hashes' pg_ripple.cdx.json
```

## Sequential migration list

| # | Script | Notes |
|---|--------|-------|
| 1 | `pg_ripple--0.1.0.sql` | Initial installation schema (v0.1.0) |
| 2 | `pg_ripple--0.1.0--0.2.0.sql` | 0.1.0 → 0.2.0 |
| 3 | `pg_ripple--0.2.0--0.3.0.sql` | 0.2.0 → 0.3.0 |
| 4 | `pg_ripple--0.3.0--0.4.0.sql` | 0.3.0 → 0.4.0 |
| 5 | `pg_ripple--0.4.0--0.5.0.sql` | 0.4.0 → 0.5.0 |
| 6 | `pg_ripple--0.5.0--0.5.1.sql` | 0.5.0 → 0.5.1 |
| 7 | `pg_ripple--0.5.1--0.6.0.sql` | 0.5.1 → 0.6.0 |
| 8 | `pg_ripple--0.6.0--0.7.0.sql` | 0.6.0 → 0.7.0 |
| 9 | `pg_ripple--0.7.0--0.8.0.sql` | 0.7.0 → 0.8.0 |
| 10 | `pg_ripple--0.8.0--0.9.0.sql` | 0.8.0 → 0.9.0 |
| 11 | `pg_ripple--0.9.0--0.10.0.sql` | 0.9.0 → 0.10.0 |
| 12 | `pg_ripple--0.10.0--0.11.0.sql` | 0.10.0 → 0.11.0 |
| 13 | `pg_ripple--0.11.0--0.12.0.sql` | 0.11.0 → 0.12.0 |
| 14 | `pg_ripple--0.12.0--0.13.0.sql` | 0.12.0 → 0.13.0 |
| 15 | `pg_ripple--0.13.0--0.14.0.sql` | 0.13.0 → 0.14.0 |
| 16 | `pg_ripple--0.14.0--0.15.0.sql` | 0.14.0 → 0.15.0 |
| 17 | `pg_ripple--0.15.0--0.16.0.sql` | 0.15.0 → 0.16.0 |
| 18 | `pg_ripple--0.16.0--0.17.0.sql` | 0.16.0 → 0.17.0 |
| 19 | `pg_ripple--0.17.0--0.18.0.sql` | 0.17.0 → 0.18.0 |
| 20 | `pg_ripple--0.18.0--0.19.0.sql` | 0.18.0 → 0.19.0 |
| 21 | `pg_ripple--0.19.0--0.20.0.sql` | 0.19.0 → 0.20.0 |
| 22 | `pg_ripple--0.20.0--0.21.0.sql` | 0.20.0 → 0.21.0 |
| 23 | `pg_ripple--0.21.0--0.22.0.sql` | 0.21.0 → 0.22.0 |
| 24 | `pg_ripple--0.22.0--0.23.0.sql` | 0.22.0 → 0.23.0 |
| 25 | `pg_ripple--0.23.0--0.24.0.sql` | 0.23.0 → 0.24.0 |
| 26 | `pg_ripple--0.24.0--0.25.0.sql` | 0.24.0 → 0.25.0 |
| 27 | `pg_ripple--0.25.0--0.26.0.sql` | 0.25.0 → 0.26.0 |
| 28 | `pg_ripple--0.26.0--0.27.0.sql` | 0.26.0 → 0.27.0 |
| 29 | `pg_ripple--0.27.0--0.28.0.sql` | 0.27.0 → 0.28.0 |
| 30 | `pg_ripple--0.28.0--0.29.0.sql` | 0.28.0 → 0.29.0 |
| 31 | `pg_ripple--0.29.0--0.30.0.sql` | 0.29.0 → 0.30.0 |
| 32 | `pg_ripple--0.30.0--0.31.0.sql` | 0.30.0 → 0.31.0 |
| 33 | `pg_ripple--0.31.0--0.32.0.sql` | 0.31.0 → 0.32.0 |
| 34 | `pg_ripple--0.32.0--0.33.0.sql` | 0.32.0 → 0.33.0 |
| 35 | `pg_ripple--0.33.0--0.34.0.sql` | 0.33.0 → 0.34.0 |
| 36 | `pg_ripple--0.34.0--0.35.0.sql` | 0.34.0 → 0.35.0 |
| 37 | `pg_ripple--0.35.0--0.36.0.sql` | 0.35.0 → 0.36.0 |
| 38 | `pg_ripple--0.36.0--0.37.0.sql` | 0.36.0 → 0.37.0 |
| 39 | `pg_ripple--0.37.0--0.38.0.sql` | 0.37.0 → 0.38.0 |
| 40 | `pg_ripple--0.38.0--0.39.0.sql` | 0.38.0 → 0.39.0 |
| 41 | `pg_ripple--0.39.0--0.40.0.sql` | 0.39.0 → 0.40.0 |
| 42 | `pg_ripple--0.40.0--0.41.0.sql` | 0.40.0 → 0.41.0 |
| 43 | `pg_ripple--0.41.0--0.42.0.sql` | 0.41.0 → 0.42.0 |
| 44 | `pg_ripple--0.42.0--0.43.0.sql` | 0.42.0 → 0.43.0 |
| 45 | `pg_ripple--0.43.0--0.44.0.sql` | 0.43.0 → 0.44.0 |
| 46 | `pg_ripple--0.44.0--0.45.0.sql` | 0.44.0 → 0.45.0 |
| 47 | `pg_ripple--0.45.0--0.46.0.sql` | 0.45.0 → 0.46.0 |
| 48 | `pg_ripple--0.46.0--0.47.0.sql` | 0.46.0 → 0.47.0 |
| 49 | `pg_ripple--0.47.0--0.48.0.sql` | 0.47.0 → 0.48.0 |
| 50 | `pg_ripple--0.48.0--0.49.0.sql` | 0.48.0 → 0.49.0 |
| 51 | `pg_ripple--0.49.0--0.50.0.sql` | 0.49.0 → 0.50.0 |
| 52 | `pg_ripple--0.50.0--0.51.0.sql` | 0.50.0 → 0.51.0 |
| 53 | `pg_ripple--0.51.0--0.52.0.sql` | 0.51.0 → 0.52.0 |
| 54 | `pg_ripple--0.52.0--0.53.0.sql` | 0.52.0 → 0.53.0 |
| 55 | `pg_ripple--0.53.0--0.54.0.sql` | 0.53.0 → 0.54.0 |
| 56 | `pg_ripple--0.54.0--0.55.0.sql` | 0.54.0 → 0.55.0 |
| 57 | `pg_ripple--0.55.0--0.56.0.sql` | 0.55.0 → 0.56.0 |
| 58 | `pg_ripple--0.56.0--0.57.0.sql` | 0.56.0 → 0.57.0 |
| 59 | `pg_ripple--0.57.0--0.58.0.sql` | 0.57.0 → 0.58.0 |
| 60 | `pg_ripple--0.58.0--0.59.0.sql` | 0.58.0 → 0.59.0 |
| 61 | `pg_ripple--0.59.0--0.60.0.sql` | 0.59.0 → 0.60.0 |
| 62 | `pg_ripple--0.60.0--0.61.0.sql` | 0.60.0 → 0.61.0 |
| 63 | `pg_ripple--0.61.0--0.62.0.sql` | 0.61.0 → 0.62.0 |
| 64 | `pg_ripple--0.62.0--0.63.0.sql` | 0.62.0 → 0.63.0 |
| 65 | `pg_ripple--0.63.0--0.64.0.sql` | 0.63.0 → 0.64.0 |
| 66 | `pg_ripple--0.64.0--0.65.0.sql` | 0.64.0 → 0.65.0 |
| 67 | `pg_ripple--0.65.0--0.66.0.sql` | 0.65.0 → 0.66.0 |
| 68 | `pg_ripple--0.66.0--0.67.0.sql` | 0.66.0 → 0.67.0 |
| 69 | `pg_ripple--0.67.0--0.68.0.sql` | 0.67.0 → 0.68.0 |
| 70 | `pg_ripple--0.68.0--0.69.0.sql` | 0.68.0 → 0.69.0 |
| 71 | `pg_ripple--0.69.0--0.70.0.sql` | 0.69.0 → 0.70.0 |
| 72 | `pg_ripple--0.70.0--0.71.0.sql` | 0.70.0 → 0.71.0 |
| 73 | `pg_ripple--0.71.0--0.72.0.sql` | 0.71.0 → 0.72.0 |
| 74 | `pg_ripple--0.72.0--0.73.0.sql` | 0.72.0 → 0.73.0 |
| 75 | `pg_ripple--0.73.0--0.74.0.sql` | 0.73.0 → 0.74.0 |
| 76 | `pg_ripple--0.74.0--0.75.0.sql` | 0.74.0 → 0.75.0 |
| 77 | `pg_ripple--0.75.0--0.76.0.sql` | 0.75.0 → 0.76.0 |
| 78 | `pg_ripple--0.76.0--0.77.0.sql` | 0.76.0 → 0.77.0 |
| 79 | `pg_ripple--0.77.0--0.78.0.sql` | 0.77.0 → 0.78.0 |
| 80 | `pg_ripple--0.78.0--0.79.0.sql` | 0.78.0 → 0.79.0 |
| 81 | `pg_ripple--0.79.0--0.80.0.sql` | 0.79.0 → 0.80.0 |
| 82 | `pg_ripple--0.80.0--0.81.0.sql` | 0.80.0 → 0.81.0 |
| 83 | `pg_ripple--0.81.0--0.82.0.sql` | 0.81.0 → 0.82.0 |
| 84 | `pg_ripple--0.82.0--0.83.0.sql` | 0.82.0 → 0.83.0 |
| 85 | `pg_ripple--0.83.0--0.84.0.sql` | 0.83.0 → 0.84.0 |
| 86 | `pg_ripple--0.84.0--0.85.0.sql` | 0.84.0 → 0.85.0 |
| 87 | `pg_ripple--0.85.0--0.86.0.sql` | 0.85.0 → 0.86.0 |
| 88 | `pg_ripple--0.86.0--0.87.0.sql` | 0.86.0 → 0.87.0 |
| 89 | `pg_ripple--0.87.0--0.88.0.sql` | 0.87.0 → 0.88.0 |
| 90 | `pg_ripple--0.88.0--0.89.0.sql` | 0.88.0 → 0.89.0 |
| 91 | `pg_ripple--0.89.0--0.90.0.sql` | 0.89.0 → 0.90.0 |
| 92 | `pg_ripple--0.90.0--0.91.0.sql` | 0.90.0 → 0.91.0 |
| 93 | `pg_ripple--0.91.0--0.92.0.sql` | 0.91.0 → 0.92.0 |
| 94 | `pg_ripple--0.92.0--0.93.0.sql` | 0.92.0 → 0.93.0 |
| 95 | `pg_ripple--0.93.0--0.94.0.sql` | 0.93.0 → 0.94.0 |
| 96 | `pg_ripple--0.94.0--0.95.0.sql` | 0.94.0 → 0.95.0 |
| 97 | `pg_ripple--0.95.0--0.96.0.sql` | 0.95.0 → 0.96.0 |
| 98 | `pg_ripple--0.96.0--0.97.0.sql` | 0.96.0 → 0.97.0 |
| 99 | `pg_ripple--0.97.0--0.98.0.sql` | 0.97.0 → 0.98.0 |
| 100 | `pg_ripple--0.98.0--0.99.0.sql` | 0.98.0 → 0.99.0 |
| 101 | `pg_ripple--0.99.0--0.99.1.sql` | 0.99.0 → 0.99.1 |
| 102 | `pg_ripple--0.99.1--0.99.2.sql` | 0.99.1 → 0.99.2 |
| 103 | `pg_ripple--0.99.2--0.100.0.sql` | 0.99.2 → 0.100.0 |
| 104 | `pg_ripple--0.100.0--0.101.0.sql` | 0.100.0 → 0.101.0 |
| 105 | `pg_ripple--0.101.0--0.102.0.sql` | 0.101.0 → 0.102.0 |
| 106 | `pg_ripple--0.102.0--0.103.0.sql` | 0.102.0 → 0.103.0 |
| 107 | `pg_ripple--0.103.0--0.104.0.sql` | 0.103.0 → 0.104.0 |
| 108 | `pg_ripple--0.104.0--0.105.0.sql` | 0.104.0 → 0.105.0 |
| 109 | `pg_ripple--0.105.0--0.106.0.sql` | 0.105.0 → 0.106.0 |
| 110 | `pg_ripple--0.106.0--0.107.0.sql` | 0.106.0 → 0.107.0 |
| 111 | `pg_ripple--0.107.0--0.108.0.sql` | 0.107.0 → 0.108.0 |
| 112 | `pg_ripple--0.108.0--0.109.0.sql` | 0.108.0 → 0.109.0 |
| 113 | `pg_ripple--0.109.0--0.110.0.sql` | 0.109.0 → 0.110.0 |
| 114 | `pg_ripple--0.110.0--0.111.0.sql` | 0.110.0 → 0.111.0 |
| 115 | `pg_ripple--0.111.0--0.112.0.sql` | 0.111.0 → 0.112.0 |
| 116 | `pg_ripple--0.112.0--0.113.0.sql` | 0.112.0 → 0.113.0 |
| 117 | `pg_ripple--0.113.0--0.114.0.sql` | 0.113.0 → 0.114.0 |
| 118 | `pg_ripple--0.114.0--0.115.0.sql` | 0.114.0 → 0.115.0 |
| 119 | `pg_ripple--0.115.0--0.116.0.sql` | 0.115.0 → 0.116.0 |
