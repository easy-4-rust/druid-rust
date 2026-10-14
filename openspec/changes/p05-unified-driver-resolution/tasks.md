## 1. Standard parser contract

- [x] 1.1 Add standard-crate tests for current subprotocol/authority syntax, percent decoding, duplicate property precedence, profile extraction, redaction, and rejected user-info/fragments; establish the expected failure before implementation.
- [x] 1.2 Implement `druid_rdbc::url::RdbcConnectionUrl` with Chinese public documentation, secret-safe `Debug`, and behavior matching the current Druid parser.

## 2. Compatibility and integration

- [x] 2.1 Keep `druid::sql::RdbcUrl` public signatures and error behavior as a facade over the standard parser; run existing URL differential tests.
- [x] 2.2 Route wrapper driver-profile parsing through the standard parser while keeping runtime-specific URL normalization in the wrapper; test profile mismatch, feature-disabled providers, and credential redaction.
- [x] 2.3 Run dependency-boundary, targeted crate tests, all-feature rustdoc, and relevant wrapper resolution tests; resolve regressions without broadening provider policy into the standard parser.

## 3. Acceptance and delivery

- [ ] 3.1 Review the public API diff and SemVer report for the additive standard type and unchanged Druid facade.
- [ ] 3.2 Run `openspec validate p05-unified-driver-resolution --strict`, sync accepted requirements, then commit and push this P05 feature point.

## 4. Remaining P05 runtime and package gates

- [x] 4.1 Inspect every production connection factory and runtime entry; confirm physical factory creation is async and no `block_on` or private runtime bridge remains.
- [x] 4.2 Verify `DriverManager::connect_registered` delegates to `DriverRegistry::connect`; run the standard negotiation and callback regression tests.
- [x] 4.3 Resolve `druid-wrapper` with `--no-default-features` and run the feature-isolation test; confirm optional runtime packages do not appear.
- [x] 4.4 Run the package publication/CI ownership verifier and confirm workspace package, binary, driver, and live-contract coverage.
