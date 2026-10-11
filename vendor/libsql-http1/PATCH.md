# libsql 0.9.30 HTTP/1-only patch

This directory contains the `libsql` 0.9.30 crates.io source under its MIT
license (`LICENSE.md`). The functional changes remove Hyper's unused `http2`
feature and replace `hyper-rustls 0.25` with `hyper-tls 0.5` for the Hyper 0.14
remote connector. `src/database.rs` creates the native TLS context fallibly and
passes it to the connector, preserving HTTP and HTTPS support without a TLS
setup panic. On Linux, native TLS uses the system OpenSSL implementation and
trust store; on Windows and macOS, it uses the platform TLS implementation.
Four upstream source/test files also have trailing whitespace removed so
`git diff --check` passes. The wrapper enables `remote` and `tls` and uses
this connector.

This removes the unused `h2 0.3.x` dependency, which has
[RUSTSEC-2026-0258](https://rustsec.org/advisories/RUSTSEC-2026-0258), and the
`rustls-webpki 0.102.x` chain, which has multiple RustSec advisories. Do not
replace this patch with audit ignores. Remove it after an upstream release
migrates libSQL to supported Hyper/TLS dependencies and the remote driver
contracts and supply-chain gate pass against that release.
