# Worldwright Changes

## Unreleased — Content Loading v0.1

Implemented the six-step content milestone in the unpublished `0.1.0-dev.0` workspace:

1. Defined strict JSON schema v1 and diagnostic contracts in `docs/content/SCHEMA_V1.md`.
2. Added bounded file/byte loading, duplicate-key/depth guards, version dispatch, semantic/reference/graph checks, and capped safe diagnostics.
3. Added private validated types, deterministic definition-to-room mapping, named-exit lookup, and all-or-nothing `WorldBuilder` conversion.
4. Added SDK/Testkit content exports and JSON-backed Look/Move/Look/Say plus one-way movement rejection tests. The original programmatic fixture and core runtime are unchanged.
5. Added `worldwright content validate <path>`, command help, output/exit-code contracts, and executable tests including outside-workspace and path handling.
6. Added committed registry dependency resolution, bootstrap fetch before offline checks, native-host default targets, documentation, and captured verification.

### Breaking unpublished API change

Removed `ContentPipelineStub` and its SDK/Testkit re-exports. Use `load_file`/`load_bytes`, then `instantiate`; SDK/Testkit expose the content crate as `content`. JSON v1 is the first implemented schema; missing/unsupported versions fail explicitly. No legacy format is silently migrated or guessed.

### Build and verification fixes

Console's smoke wrapper was under `test/`, which Cargo does not discover. Moved it to `tests/` without changing its assertions. The default Windows MSVC target was removed so the supplied Linux/Windows matrix uses native hosts. Root package versions and the Rust 1.85.1 pin are unchanged. Source/provenance and platform limits are recorded in [verification](docs/VERIFICATION.md).

No listener, UI, database, live activation, release signing, or authentication is added.

## Historical — World Runtime v0.1

This is an increment to the unpublished `0.1.0-dev.0` workspace, not a tagged release or stable SDK promise.

### Added

- Core `runtime` module with validated `WorldBuilder`, private world state, owned snapshots, trusted-host attachment management, and synchronous `WorldRuntime::execute`.
- Directed exits and movement enforcement independent of a rule's proposed effects.
- Staged, all-or-nothing in-memory state changes and buffered output; failures return no events/messages.
- Setting-neutral reference Look/Move/Say rules and a real programmatic two-room Testkit fixture.
- Scoped rule views, deterministic recipient resolution, checked revisions/generations, and delivery/build limits.
- 24 end-to-end runtime tests, increasing the observed workspace total from 48 to 72 passing tests.
- `docs/RUNTIME.md` and a captured runtime verification log.

### Changed

- Extended `WorldView` with conservative default description/traversal queries and an owned `RoomDescription` type.
- Re-exported the runtime through the SDK and updated its status description.
- Clarified server help: local SDK execution exists, but default network-service startup remains unsupported.
- Updated README, design, architecture, security boundaries, test guidance, and verification documentation.
- Updated workspace stage metadata. No crate, Rust dependency, package version, toolchain pin, wire schema, configuration format, or database migration was added or changed.

### Explicit limits

- `attach` is a trusted-host API, not account authentication.
- Operation IDs do not provide duplicate suppression; results are not durable receipts.
- Runtime generations do not provide restart-safe session epochs.
- Item transfer and scheduling effects are rejected, not silently ignored.
- No interactive client, network listener, UI, content loader, persistence, or deployed service was added.
- Rust rules and caller RNG are not transactional resources or a security sandbox.

### Verification

Linux Rust 1.85.0: formatting, all-target checks, Clippy with warnings denied, 72 tests, debug build, and API documentation passed. Exact pinned Rust 1.85.1, Windows, and hosted CI remain pending. See [verification](docs/VERIFICATION.md) for toolchain provenance and [runtime guide](docs/RUNTIME.md) for the implemented contract.
