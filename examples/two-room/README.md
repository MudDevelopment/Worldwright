# Two-room JSON reference world

`world.json` defines Atrium and Observatory, with explicit north/south directed exits. Character names, accounts, connections, and rule registration are deliberately absent: the trusted host supplies those.

From the repository root, after bootstrap:

```bash
cargo run -p worldwright --locked --offline -- content validate examples/two-room/world.json
cargo test -p worldwright-testkit --test content_runtime --locked --offline
```

The runtime test loads this exact file, creates Alice/Bob through the trusted builder API, and verifies Look, Move, Look, Say, exact recipients, and rejection/rollback for reverse movement in a one-way variant. The original programmatic fixture remains independent of JSON loading.

Edit a title or description and rerun validation. Introduce an unknown exit destination to see a JSON-pointer diagnostic and exit code 1. See [schema v1](../../docs/content/SCHEMA_V1.md) for all restrictions. Validation does not launch an interactive game or approve a release.
