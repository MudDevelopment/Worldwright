# Reference Examples

The examples contain original schema-v1 fixtures. Validate them with the
shared content loader:

```text
cargo run -p worldwright -- content validate examples/two-room/world.json
cargo run -p worldwright -- content validate examples/expedition/world.json
```

Keep application binaries under `apps/`. Add examples only after the relevant format is implemented, validated, and its license/provenance is clear. Do not copy copyrighted legacy world databases here.
