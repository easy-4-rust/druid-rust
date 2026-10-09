# druid-rust compatibility patch

This directory vendors the source logic of `toasty-driver-sqlite 0.9.0`
from <https://github.com/tokio-rs/toasty>, licensed under MIT.

The only intentional dependency change is `rusqlite 0.40` to `rusqlite 0.32.1`
so Toasty and SQLx 0.8 share `libsqlite3-sys 0.30.1` in one Cargo graph.
The patch is gated by real SQLite integration tests in `druid-toasty`,
`druid`, `druid-sqlx`, bb8, deadpool, and `druid-wrapper`.

Remove this patch when the upstream Toasty and SQLx release lines accept a
common `libsqlite3-sys`.

## Native result metadata handoff

The local driver also preserves actual prepared-statement column labels and
declared types in a per-connection `SqliteQueryMetadata` snapshot.
`connect_with_query_metadata` creates the same native connection using the
configured `Sqlite` instance and official `ConnectContext`. It does not expose
a second SQL executor. Official `Connection::exec` remains the execution path.
Each prepare clears the prior snapshot; the adapter takes the successful query
snapshot under its existing connection lock, including empty result sets.
The adapter removes the internal RETURNING key column alongside row extraction.
The real integration controls are `toasty_native_metadata_contract_test` and
`standard_buffered_stream_contract_test` in `druid-wrapper`. Their terminal
results are recorded in the RDBC verification reports; source presence alone
is not proof of the patch passing.
