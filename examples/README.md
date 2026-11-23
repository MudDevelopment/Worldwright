# Reference Examples

The examples contain original schema-v1 fixtures. Validate them with the
shared content loader:

```text
cargo run -p worldwright -- content validate examples/two-room/world.json
cargo run -p worldwright -- content validate examples/expedition/world.json
cargo run -p worldwright -- content validate examples/karad-umbar/world.json
```

[Karad-Umbar](karad-umbar/README.md) is a 100-room MUD world using reference rules. Its two-witness quest can also be compiled into seeded, kingdom-wide expeditions.

Keep application binaries under `apps/`. Add examples only after the relevant format is implemented, validated, and its license/provenance is clear. Do not copy copyrighted legacy world databases here.
