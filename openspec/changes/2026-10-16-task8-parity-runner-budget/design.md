# Design

`run_checked` accepts `None` as an explicit outer timeout. Only the Task 6 method-parity invocation uses it. The child retains its bounded subprocess timeouts and propagates nonzero results to the parent. Other parent commands keep their current finite budgets. A verifier self-test asserts this composition; full-gate execution remains necessary before Task 8 can be declared complete.
