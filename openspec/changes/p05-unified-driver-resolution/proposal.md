# P05: unify RDBC URL resolution

## Why

RDBC product URLs are parsed by `druid::sql::RdbcUrl`, while the standard `druid-rdbc` crate currently exposes a different `RdbcUrl` value for JDBC URL parameters. Wrapper and pool-builder code calls the Druid parser directly, and legacy JDBC-to-native DSN conversion has a separate adapter. The split makes it difficult for standard callers to share the same validation, property decoding, and redaction rules.

## What changes

- Add an unambiguous standard `RdbcConnectionUrl` parser to `druid-rdbc` for `rdbc:` product URLs.
- Keep the existing public `druid::sql::RdbcUrl` API as a compatibility facade over the standard parser.
- Route wrapper and driver-profile resolution through the standard parser while retaining provider-specific URL normalization and legacy JDBC conversion at their current boundaries.
- Preserve current accepted URL forms, property precedence, validation errors, and credential redaction.

## Scope and non-goals

This change covers P05's unified parser entry point only. It does not remove JDBC/native DSN adapters, alter driver negotiation, change public Druid method signatures, or claim the remaining P05–P07 tasks complete. Optional dependency isolation and publication/CI ownership are separate acceptance items.

## Acceptance

- The standard parser is the only implementation of product-profile RDBC URL syntax and validation.
- Existing Druid URL parsing behavior and public signatures remain compatible.
- Wrapper/profile resolution uses the standard parser; native driver-specific URL conversion remains runtime-owned.
- Tests cover both supported RDBC URL spellings, encoded properties, duplicate properties, mismatched profiles, user-info, fragments, empty endpoints, and redaction.
- The standard crate's dependency-boundary verifier, targeted RDBC URL tests, wrapper resolution tests, strict rustdoc, and OpenSpec validation pass.
- This feature point is committed and pushed only after these checks pass.
