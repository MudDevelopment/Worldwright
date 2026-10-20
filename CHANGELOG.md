# Worldwright Changes

## Unreleased — World Runtime v0.1

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
