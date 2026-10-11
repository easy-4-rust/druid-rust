# libsql 0.9.30 HTTP/1-only patch

This directory contains the `libsql` 0.9.30 crates.io source under its MIT
license (`LICENSE.md`). The functional change is in `Cargo.toml` and
`Cargo.toml.orig`: remove Hyper's `http2` feature. Four upstream source/test
files also have trailing whitespace removed so `git diff --check` passes.
The remote connector in
`src/database.rs` calls `enable_http1()` and does not enable HTTP/2. The wrapper
enables `remote` and `tls` and uses this connector.

This removes the unused `h2 0.3.x` dependency, which has
[RUSTSEC-2026-0258](https://rustsec.org/advisories/RUSTSEC-2026-0258). Do not
replace this patch with an audit ignore. Remove it after an upstream release
migrates libSQL to a supported Hyper/h2 dependency and the remote driver
contracts and supply-chain gate pass against that release.
