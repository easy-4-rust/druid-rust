# Verification in progress

- New alias test: failed before the policy change because `databaseUrl` leaked; passed afterward.
- `cargo test -p druid-admin --lib --locked --offline`: 6 passed, 0 failed.
- `rustfmt --check --edition 2021 crates/druid-admin/src/security/admin_redaction_policy.rs`: passed.
- `openspec validate 2026-10-15-admin-redaction-aliases --strict`: passed.
- Full local Admin suite: 15/17 `admin_binary_test` cases passed; two TLS startup cases timed out, including in a serial rerun. A separately launched Admin process listened on both configured loopback ports, while `curl` and `Test-NetConnection` could not establish TCP to the local listener. This local network limitation leaves the full suite unverified; CI must run on the pushed source before archiving this change.
