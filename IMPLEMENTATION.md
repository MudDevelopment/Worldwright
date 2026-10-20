# Content Loading v0.1 implementation handoff

All six requested implementation steps are complete in this source snapshot:

1. **Contract** — `docs/content/SCHEMA_V1.md`: explicit v1, strict JSON objects/fields, graph and text rules, limits, diagnostics, version policy.
2. **Parsing and validation** — `crates/worldwright-content/src/{schema,loader,validation,diagnostic}.rs`: bounded input, duplicate/depth preflight, version dispatch, capped errors and directed reachability.
3. **Validated types and conversion** — `validated.rs`, `instantiate.rs`: private content, deterministic checked IDs, named-exit lookup, all-room-before-exit builder construction, no partial return.
4. **SDK and runtime proof** — SDK/Testkit `content` exports, independent existing programmatic fixture, new `content_runtime` integration tests loading the committed JSON example.
5. **CLI** — `apps/worldwright/src/cli.rs` and `commands/`: `content validate <path>`, help, safe diagnostics, file and usage errors, binary-level tests.
6. **Provisioning, documentation, verification** — normal registry dependencies/lockfile, bootstrap fetch, offline checks, host-native target, README/design/architecture/security/schema updates, captured Linux workflow logs.

## Run

```bash
bash scripts/dev.sh bootstrap
bash scripts/dev.sh check
cargo run -p worldwright --locked --offline -- content validate examples/two-room/world.json
```

On Windows use `./scripts/dev.ps1 bootstrap` then `./scripts/dev.ps1 check` before the same Cargo command.

## Important notes

- `ContentPipelineStub` was removed; this is a breaking unpublished Rust API change. Use `load_file`/`load_bytes` then `instantiate`.
- Schema v1 is the first supported content format. Missing/future versions fail explicitly; no migration is guessed.
- The original Console smoke wrapper was under `test/`; it is now under Cargo's discoverable `tests/` directory with assertions unchanged.
- No core runtime Rust source was changed. No listener, UI, persistence, login, or live activation was added.
- **103 tests passed**, plus formatting, compilation, Clippy, build, and rustdoc. Read [verification](docs/verification/content-report.md): local checks used Rust 1.85.1 npm repackages and GitHub dependency source snapshots, not approved registry tarballs. Windows and hosted CI remain unverified.
- ZIP contains source, lockfile, example, documentation, and logs; it excludes `.git`, `target`, and temporary verification tooling/caches.
