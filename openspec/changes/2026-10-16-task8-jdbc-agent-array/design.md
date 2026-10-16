# Design

The Agent session owns each driver `java.sql.Array` by an opaque random handle. A resource request resolves the handle only inside the originating session. Explicit free removes the handle only after driver `free()` succeeds, allowing a failed free to be retried. Session/logical close drains child cursors before arrays and aggregates close failures without skipping the physical connection close.

Rust `JdbcAgentConnection` must use its existing serialized `StandardExecutor` control path so Array calls still work after statement creation transfers the transport into the executor. The Rust Array implementation advertises only operations backed by protocol evidence. Array result-set cursors remain owner-bound and are closed before their parent Array.
