# Design

Keep this as an adapter-level regression test beside `SqlxConnectionAdapter`'s existing unit tests. Connect using `SqlxConnectionAdapter::connect("sqlite::memory:")`, then exercise the public `druid_rdbc::connection::Connection` and `ResultSet` traits against that live driver connection. Test connection capability truth and unsupported resource-factory behavior before executing the query. Check SQLState instead of matching backend-specific error text. Close the result set and connection explicitly.

No production behavior changes are expected: default `Connection` resource factories already return `feature_not_supported`, and the adapter's capability descriptor already reports its supported connection capabilities.
