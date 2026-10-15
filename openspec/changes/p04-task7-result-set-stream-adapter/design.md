# Design

Keep the production cursor implementation unchanged. Construct a cursor in its module test with a pending oneshot response, poll `next` once until it is pending, then drop that future by ending the poll. Deliver a row through the same response channel and invoke `next` again. Assert that the row is accepted and the cursor row number advances exactly once. Close the cursor and its SQLite actor explicitly.

The existing SQLx SQLite integration contract remains the backend-level proof for actual capabilities, unsupported factories, ordered row iteration, EOF, and explicit close. Keep its claims scoped to SQLx SQLite; other drivers need their own executed integration evidence.
