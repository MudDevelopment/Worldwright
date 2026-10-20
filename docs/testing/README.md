# Testing Roadmap

Existing tests cover typed values, bounded plans, scheduling, rule/module registration and planning, deterministic fixtures, ID separation, and retained stub behavior. See [shared test support](../../tests/README.md) and the [verification record](../VERIFICATION.md).

Next: validated two-room command execution, invalid content, then transport and authority tests. Database, retry, commit-uncertainty, writer-fencing, and recovery tests arrive with real persistence. Do not turn a stub's `NotImplemented` result into a passing gameplay scenario.
