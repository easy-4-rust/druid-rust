# Task 8 parity runner budget

## Why

The final no-physical-standard gate aborts Task 6 exact method parity after 1,800 seconds even though that child runs several independently bounded integration targets. A cold Windows run reached the outer deadline before the child could report its own result. Let the child enforce its per-command deadlines while the parent waits for its final exit status. Keep every existing evidence and failure condition unchanged.

## What Changes

The parent accepts an explicit unbounded wait for this child only; the child's test commands retain their finite timeouts and nonzero failures still fail the parent gate.
