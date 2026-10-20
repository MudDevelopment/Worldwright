# Testing Roadmap

Existing tests verify stub behavior only; see [shared test support](../../tests/README.md) and the [verification record](../VERIFICATION.md).

Next: typed ID/command tests, two-room traversal, invalid content, then bounded transport and authority tests. Database, retry, commit-uncertainty, writer-fencing, and recovery tests arrive with real persistence. Do not turn a stub's `NotImplemented` result into a passing gameplay scenario.
