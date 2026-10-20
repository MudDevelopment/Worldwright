# Testing Roadmap

Existing tests cover typed values, bounded plans, scheduling, rule/module registration and planning, deterministic fixtures, ID separation, and retained stub behavior. See [shared test support](../../tests/README.md) and the [verification record](../VERIFICATION.md).

The new `--test runtime` suite has 24 end-to-end tests for real local execution: two-room actions, context validation, staged effects, rollback, visibility, message routing, and limits. See [RUNTIME.md](../RUNTIME.md).

Current coverage includes the strict content loader, Local Play adapter, authenticated Transport v0.1 owner/session/protocol tests, and real loopback WebSocket acceptance cases. Next: persistence, retry/commit-uncertainty, writer-fencing, durable recovery, and restart tests. A passing local execution or transport result is not a durable receipt.
