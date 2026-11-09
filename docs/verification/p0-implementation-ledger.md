# P0 implementation ledger — November 9, 2026

Requested scope: WWV2-001, 002, 003, 004, 008, 009. Files are installed directly
in E:\Hold\Worldwright through EWW. Source implementation and acceptance evidence
are tracked separately. Prior Windows passes do not verify this patch.

| Item / track | Status | Evidence / remaining gate |
|---|---|---|
| WWV2-001 source | ✅ verified | Default XDG parent preparation; private owner/mode and path checks, no automatic chmod of old folders. Synthetic launcher contracts passed. |
| WWV2-001 real acceptance | ⏳ not yet checked | Fresh HOME and XDG real Studio/package/reopen/SIGTERM tests authored and CI-wired; both skipped locally because native binaries are unavailable. |
| WWV2-002 source | ✅ verified | Doctor reports host/executable/sysroot/config and redacted override presence. check-native enforces pinned rustc/Cargo, fresh native output, locked/offline phases, first-failure exit. Bash contracts passed; PowerShell source inspected. |
| WWV2-002 real acceptance | ⏳ not yet checked | Current Rustfmt/compiler/Clippy/test/build/rustdoc and Windows PowerShell/Ubuntu CI runs remain unobserved. |
| WWV2-003 source | ✅ verified | Existing game-aware host/preview/Studio retained. Added actual public supervisor selected-game/immutability and unsupported-game tests plus HTTP package contract tests; Rust grammar passed. |
| WWV2-003 real acceptance | ⏳ not yet checked | Run new Rust tests and real Studio smoke; exact-artifact deployment/live PG parity and manual browser evidence remain open. |
| WWV2-004 source | ✅ verified | UUID producer retained. Node fixture passes matching-sequence reload/new-tab IDs, reconnect, stale stream/timer isolation, no automatic receipt retries and logout. Focused ignored PostgreSQL receipt lifecycle test authored; CI verifies its name before execution. |
| WWV2-004 real acceptance | ⏳ not yet checked | Execute PostgreSQL test; real browser/new-tab/persistent server exercise and hosted job result remain unobserved. |
| WWV2-008 source/schema | ✅ verified | Draft-07 conditional OIDC activation; disabled/omitted fields no longer require enabled settings. 27 JSON Schema cases passed; other mandatory deployment gates still fail closed. |
| WWV2-008 Helm acceptance | ⏳ not yet checked | Actual Helm positive/negative matrix configured; Helm not installed in sandbox. |
| WWV2-009 source/checker | ✅ verified | Shared complete-values lint/YAML harness, actual secretName/secretKeyRef assertions, native exits, missing-tool failure; five checker unit tests passed. Ubuntu/Windows chart jobs configured with verified official action SHA and Helm tool pin. |
| WWV2-009 real acceptance | ⏳ not yet checked | Actual Helm 3 lint/render on both hosts and hosted job outcomes remain unobserved; no cluster acceptance claimed. |

## Checks observed in sandbox

- Eight synthetic launcher/diagnostic tests passed; two explicit real-binary tests skipped (not passed).
- Five synthetic YAML/native-exit checker unit tests passed.
- Twenty-seven JSON Schema cases passed using test requirements in a venv.
- Node browser lifecycle fixture passed; Bash/JavaScript/Python syntax checks passed.
- Grammar parsing passed for three touched Rust files; this is NOT Rust type checking.
- CI YAML/job wiring parsed. A full-gate invocation without Helm returned failure,
  not SKIP/PASS. That is failure-policy proof, not a Helm render result.

## Remaining execution evidence

Six of six source tracks implemented; six of six full acceptance tracks remain
open. No complete P0 closure is claimed. Cargo/rustfmt, PowerShell, Helm, real
Studio/CLI binaries, PostgreSQL and hosted workflow execution are not available
through EWW/sandbox. See ../P0_ACCEPTANCE.md for explicit required commands.
No application config/draft/artifact/DB schema migration or registry dependency
upgrade was performed. Branch protection settings are external to these files;
maintainers must require the new jobs before claiming a protected release gate.
