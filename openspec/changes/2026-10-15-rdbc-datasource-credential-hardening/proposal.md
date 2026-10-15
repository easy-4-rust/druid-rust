# Harden ordinary RDBC data-source credentials

## Why

`DruidRdbcDataSource::with_credentials` currently retains the supplied username and password as `String`s for the entire data-source lifetime and compares them with short-circuit string equality on each login attempt. This unnecessarily keeps plaintext credentials in the long-lived wrapper and makes the matching path depend on the first differing byte.

## What Changes

- Store a fixed-length, domain-separated digest of the configured credential pair instead of the plaintext pair.
- Compare fixed-length candidate digests in constant time while preserving the existing SQLState `28000` rejection and single-pool connection behavior.
- Provide a deliberately redacted `Debug` representation and regression coverage for accepted/rejected credentials and diagnostics.

## Scope

Only the ordinary `DruidRdbcDataSource` credential gate changes. Driver/database authentication, the JDBC Agent XA path, and the broader Task 8 semantic-inventory closure remain separate work.
