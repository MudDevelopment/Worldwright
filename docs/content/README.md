# Content Development

[Schema v1](SCHEMA_V1.md) is the implemented contract for a single JSON world file. It defines IDs, room text, directed named exits, entry point, intentional unreachable rooms, limits, and diagnostics. Publication and Studio editing remain future work.

After [bootstrapping](../DEVELOPMENT.md), run from the workspace root:

```bash
cargo run -p worldwright --locked --offline -- content validate examples/two-room/world.json
cargo test -p worldwright-testkit --test content_runtime --locked --offline
```

Exit codes: 0 validated and builder-conversion preflight succeeded; 1 file/content failure; 2 usage error. Paths are relative to the current working directory, not the executable. Use an absolute path when running elsewhere.

The API is available directly through `worldwright_content` or `worldwright_sdk::content`. Call `load_file` or `load_bytes` with `ContentLimits::default()`, then `instantiate` with a host-owned world ID. The result has `into_parts()` for a builder and read-only `WorldLookup`; the host adds characters, registers rules, builds, and attaches actors. See the [reference example](../../examples/two-room/README.md) and [runtime integration test](../../crates/worldwright-testkit/tests/content_runtime.rs).

`ContentPipelineStub` has been removed from this unpublished API. It is not retained as a fake-success compatibility shim. Replace old uses with the real load/instantiate flow. No prior deployed JSON schema requires migration; missing/future versions are rejected explicitly.
