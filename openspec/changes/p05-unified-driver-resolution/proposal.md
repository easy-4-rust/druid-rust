# P05: unify RDBC URL resolution

## Why

RDBC product URLs are parsed by `druid::sql::RdbcUrl`, while the standard `druid-rdbc` crate currently exposes a different `RdbcUrl` value for JDBC URL parameters. Wrapper and pool-builder code calls the Druid parser directly, and legacy JDBC-to-native DSN conversion has a separate adapter. The split makes it difficult for standard callers to share the same validation, property decoding, and redaction rules.

## What changes

- Add an unambiguous standard `RdbcConnectionUrl` parser to `druid-rdbc` for `rdbc:` product URLs.
- Keep the existing public `druid::sql::RdbcUrl` API as a compatibility facade over the standard parser.
- Route wrapper and driver-profile resolution through the standard parser while retaining provider-specific URL normalization and legacy JDBC conversion at their current boundaries.
- Preserve current accepted URL forms, property precedence, validation errors, and credential redaction.
- Verify physical connection factories remain async and do not create or block an executor inside the runtime.
- Verify the compatibility `DriverManager` delegates connection negotiation to the standard `DriverRegistry` and optional adapter dependencies stay feature-gated.
- Re-run package publication and CI ownership validation for the actual workflow matrix.

## Scope and non-goals

This change covers the five P05 acceptance items only. It does not remove JDBC/native DSN adapters, change public Druid method signatures, or claim P04 Tasks 7–8 or P06–P07 complete. Existing `rdbc-driver-negotiation` behavior remains governed by its current specification.

## Acceptance

- The standard parser is the only implementation of product-profile RDBC URL syntax and validation.
- Existing Druid URL parsing behavior and public signatures remain compatible.
- Wrapper/profile resolution uses the standard parser; native driver-specific URL conversion remains runtime-owned.
- Physical connection creation remains async end to end without a synchronous `block_on` bridge or hidden runtime construction.
- `DriverManager` connection negotiation delegates to the standard registry, and driver-specific runtime dependencies are absent from the no-default-feature dependency graph.
- Package ownership validation covers every workspace package, binary, driver matrix, and live-contract invocation.
- Tests cover both supported RDBC URL spellings, encoded properties, duplicate properties, mismatched profiles, user-info, fragments, empty endpoints, and redaction.
- The standard crate's dependency-boundary verifier, targeted RDBC URL and DriverManager tests, wrapper resolution and feature isolation tests, strict rustdoc, package ownership verifier, and OpenSpec validation pass.
- This feature point is committed and pushed only after these checks pass.
