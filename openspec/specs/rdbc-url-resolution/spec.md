# rdbc-url-resolution Specification

## Purpose
Provide one standard entry point for parsing product-profile RDBC connection URLs while preserving the existing Druid compatibility surface and keeping JDBC/native driver adaptation in the selected provider.

## Requirements

### Requirement: Standard product URL parsing

The `druid-rdbc` standard contract SHALL expose a product-profile URL parser that validates `rdbc:` URLs, extracts the profile, endpoint, database, and decoded connection properties, and produces a credential-free diagnostic representation.

#### Scenario: Parse subprotocol form
- **WHEN** a caller parses `rdbc:<profile>://<endpoint>/<database>?<properties>`
- **THEN** the parser returns the selected profile, endpoint, database, and URL-decoded properties without exposing query values in diagnostics

#### Scenario: Parse authority form
- **WHEN** a caller parses `rdbc://<profile>/<endpoint>/<database>?<properties>`
- **THEN** the parser returns the same semantic projection while preserving the source form in its canonical redacted representation

#### Scenario: Reject credentials embedded in the URL
- **WHEN** a product URL contains user-info or a fragment
- **THEN** parsing fails with an invalid-argument classification and diagnostic output does not echo credential-bearing input

### Requirement: Compatibility facade preserves behavior

The existing `druid::sql::RdbcUrl` public API SHALL delegate product URL parsing to the standard parser and preserve its public method signatures, accepted forms, decoded property behavior, redaction, and observable validation messages.

#### Scenario: Existing Druid callers parse a valid URL
- **WHEN** existing code calls `druid::sql::RdbcUrl::parse`
- **THEN** it receives the same profile, endpoint, database, properties, and redacted form as before

#### Scenario: Existing Druid callers receive invalid input
- **WHEN** existing code passes malformed, credential-bearing, or profile-less input
- **THEN** it receives the same `DruidError::InvalidArgument` behavior without leaking the raw URL

### Requirement: Runtime-specific URL policy remains above parsing

Product URL parsing SHALL remain separate from runtime-specific native DSN normalization and legacy JDBC URL translation; profile resolution SHALL validate the parsed profile and select the corresponding runtime adapter.

#### Scenario: Resolve a product URL for a runtime
- **WHEN** the profile resolver accepts a parsed product URL
- **THEN** it applies profile/runtime URL rules after parsing and passes only the runtime-compatible URL to the selected provider

### Requirement: Physical factory creation remains asynchronous

Physical connection factories SHALL expose asynchronous creation and SHALL NOT synchronously block on an executor or create a private runtime to bridge asynchronous driver I/O.

#### Scenario: Factory opens a physical connection
- **WHEN** a pool asks a provider factory to create a physical connection
- **THEN** the operation is awaited through the caller's runtime and any I/O error is returned through the standard result

### Requirement: Optional driver runtimes stay isolated

Driver-specific runtime dependencies SHALL be enabled only by their declared Cargo features; a no-default-feature wrapper build SHALL NOT resolve SQLx, DuckDB, HTTP SQL, libSQL, or Toasty runtime dependencies.

#### Scenario: Build wrapper without driver features
- **WHEN** Cargo resolves `druid-wrapper` with `--no-default-features`
- **THEN** optional database runtime packages are absent from the normal dependency graph and the feature-isolation contract test passes
