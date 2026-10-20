# Architecture Decisions

Use the decision register in [DESIGN.md](../../DESIGN.md) until individual records are needed. A record should identify the problem, alternatives, decision, affected contracts, migration impact, and verification.

Bootstrap decision: four applications and six libraries are explicitly scaffolded at the user's request. They use only workspace path dependencies and the Rust standard library. Rust 1.85.1 / edition 2024 is a selected bootstrap toolchain, not a tested compatibility matrix. External runtime dependencies remain deferred.
