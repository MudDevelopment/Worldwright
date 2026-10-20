# Content Loading v0.1 — Verification

## Observed results

The complete Bash bootstrap and check tasks ran from outside the repository and returned exit code 0. Evidence:

- [Bootstrap log](content-bootstrap-linux-rust-1.85.1.log)
- [Full check log](content-linux-rust-1.85.1.log)

| Check | Observed result |
|---|---|
| Cargo lock generation and locked dependency resolution | Passed; 10 workspace packages and 12 registry packages |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --all-targets --locked --offline` | Passed |
| Clippy, all targets/features, warnings denied | Passed |
| Unit and integration tests | 101 passed, zero failures |
| Compile-fail documentation tests | 2 passed |
| Total | **103 passed; 0 failed** |
| `cargo build --workspace --locked --offline` | Passed |
| API documentation with `-D warnings` | Passed |
| Bootstrap/check outside workspace | Passed |
| Markdown local file links and JSON code blocks | Checked; no missing file links or malformed JSON examples |
| CLI file/path/error/exit-code tests | Passed, including spaces, non-UTF-8 Linux paths, terminal controls, missing/oversized files |
| Original 24 runtime integration tests | Preserved and passed |
| Console smoke test discovery | Corrected `test/` -> `tests/`; 5 unchanged assertions now run |

## Actual environment and provenance

- x86_64 Linux; `rustc 1.85.1 (4eb161250 2025-03-15)`.
- `cargo 1.85.1 (d73d2caf9 2024-12-31)`.
- Rustc, Cargo, standard library, Clippy, and rustfmt were obtained from matching third-party `@rustbin/*-1.85.1-x86_64-unknown-linux-gnu` npm repackages, with npm install scripts disabled. They were not independently signature-verified and are not the project's approved tool distribution channel.
- crates.io and static.crates.io DNS resolution failed in this sandbox. The build therefore used a **temporary Cargo directory-source replacement containing matching upstream GitHub source tags**, not downloaded/published registry `.crate` artifacts.
- Serde derive's workspace-inherited dependency entries were expanded from its tagged upstream workspace manifest for the temporary source cache. No dependency Rust source was edited. Per-file hashes were generated for these snapshots; these are not a claim of equivalence to registry artifact bytes.
- Cargo generated the delivered lockfile using those source versions. It retains standard crates.io source identities and package checksums taken from the official crates.io index on GitHub. The temporary source-replacement configuration is outside the repository and is **not delivered**. The project uses ordinary registry dependencies and `cargo fetch --locked` on a normal machine.
- The local build disabled debug symbols and incremental compilation through environment overrides to fit sandbox storage. Release optimization, Windows linking, and performance were not tested.

### Temporary dependency snapshot versions

| Crate(s) | Upstream repository / tag |
|---|---|
| serde, serde_core, serde_derive 1.0.228 | serde-rs/serde, `v1.0.228` |
| serde_json 1.0.145 | serde-rs/json, `v1.0.145` |
| serde_path_to_error 0.1.20 | dtolnay/path-to-error, `0.1.20` |
| itoa 1.0.15 | dtolnay/itoa, `1.0.15` |
| ryu 1.0.20 | dtolnay/ryu, `1.0.20` |
| memchr 2.7.6 | BurntSushi/memchr, `2.7.6` |
| proc-macro2 1.0.101 | dtolnay/proc-macro2, `1.0.101` |
| quote 1.0.40 | dtolnay/quote, `1.0.40` |
| syn 2.0.106 | dtolnay/syn, `2.0.106` |
| unicode-ident 1.0.19 | dtolnay/unicode-ident, `1.0.19` |

## Remaining acceptance gates

1. Run bootstrap/check on approved Rust 1.85.1 with approved registry artifacts. The local source-snapshot checks are genuine, but do not prove registry artifact provenance or a fresh normal-registry build.
2. Run the supplied Windows PowerShell workflow and both hosted CI jobs. Neither Windows nor remote CI was available here.
3. Network authentication, persistence, live activation, release signing, UI, interactive play, and production safety remain outside this milestone.

The source ZIP excludes Git internals, build output, toolchains, dependency caches, and temporary source replacement. It includes the source, lockfile, example, documentation, and logs. Historical runtime logs/reports are retained as history, not evidence for the current source.
