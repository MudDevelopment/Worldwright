# Expedition example

This original three-location world supports the independent turn-based
`worldwright-expedition` rules.

```text
cargo run -p worldwright -- content validate examples/expedition/world.json
cargo test -p worldwright-expedition
```

Expedition resources, encounters, progression, and delayed recovery are
in-memory development behavior. They do not yet claim PostgreSQL durability.
