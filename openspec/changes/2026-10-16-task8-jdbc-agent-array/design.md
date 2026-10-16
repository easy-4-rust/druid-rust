# Design

The Agent session owns each driver `java.sql.Array` by an opaque random handle. A resource request resolves the handle only inside the originating session. Explicit free removes the handle only after driver `free()` succeeds, allowing a failed free to be retried. Session/logical close drains child cursors before arrays and aggregates close failures without skipping the physical connection close.

Rust `JdbcAgentConnection` must use its existing serialized `StandardExecutor` control path so Array calls still work after statement creation transfers the transport into the executor. The Rust Array implementation advertises only operations backed by protocol evidence. Array result-set cursors remain owner-bound and are closed before their parent Array.

Array elements carry the concrete Java numeric wrapper kind where `RdbcObject` specifies one. The generic `Value::Int` retains the existing protocol `int(i64)` representation; `RdbcObject::Integer` uses `integer(i32)` so H2 and other drivers can distinguish SQL `INTEGER` from `BIGINT` during `createArrayOf`.

The Rust adapter advertises READ, RANGE, RESULT_SET and FREE after real-H2 verification. Its forward-only Array cursor requests eight rows per page, retains an in-flight page future across cancellation of `next`, and exposes the driver's index/value columns without eager materialization. Parent `free()` and pooled owner close invalidate child cursors; the remote Agent drains children before the parent Array. Type-map overloads remain explicitly unsupported with SQLState `0A000`. Cancellation during a pending remote page still requires a controlled regression test before lifecycle coverage is complete.
