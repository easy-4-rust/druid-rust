# Design

Use each adapter's existing deterministic factory test to obtain a real standard `Box<dyn Connection>` without a network dependency. Before closing it, compare the full `ConnectionCapabilities` value with its all-false default; call `set_auto_commit(false)` and `set_savepoint_named` to prove transaction/savepoint rejection, `prepare_call` to prove callable rejection, and `create_array_of` to prove SQLState `0A000`. The tests run in the existing workspace all-features CI job.
