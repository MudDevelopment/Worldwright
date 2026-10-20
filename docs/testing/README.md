# Testing Roadmap

Existing tests cover typed values, bounded plans, scheduling, rule/module registration and planning, deterministic fixtures, ID separation, and retained stub behavior. See [shared test support](../../tests/README.md) and the [verification record](../VERIFICATION.md).

The new `--test runtime` suite has 24 end-to-end tests for real local execution: two-room actions, context validation, staged effects, rollback, visibility, message routing, and limits. See [RUNTIME.md](../RUNTIME.md).

Next: a versioned content loader and invalid-content tests, then an interactive adapter and authenticated transport. Database, retry, commit-uncertainty, writer-fencing, and recovery tests arrive with real persistence. A passing local execution is not a durable receipt.
