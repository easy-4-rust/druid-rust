# druid-rust compatibility patch

This directory vendors the source logic of `toasty-driver-sqlite 0.9.0`
from <https://github.com/tokio-rs/toasty>, licensed under MIT.

The only intentional dependency change is `rusqlite 0.40` to `rusqlite 0.32.1`
so Toasty and SQLx 0.8 share `libsqlite3-sys 0.30.1` in one Cargo graph.
The patch is gated by real SQLite integration tests in `druid-toasty`,
`druid`, `druid-sqlx`, bb8, deadpool, and `druid-wrapper`.

Remove this patch when the upstream Toasty and SQLx release lines accept a
common `libsqlite3-sys`.
