# Playable expedition — executable, preview and multiplayer

Implementation update: November 9, 2026. Source changes are installed through EWW;
current Cargo formatting/compile/Clippy/Rust-test execution is pending. Earlier
Windows passes predate these changes and do not verify this patch.

## One rules implementation, multiple real entry points

`crates/worldwright-expedition/src/lib.rs` remains the game authority. Supplies,
threat, encounters, strikes, rest, victory ranks and scheduled recovery are not
reimplemented in browser JavaScript. The new `terminal.rs` hosts those rules for
generated executables and official expedition artifact preview. The multiplayer
owner already selected the same rules and initialized counters with
`--game expedition`; its existing PostgreSQL commit path is retained.

| Command | Meaning |
|---|---|
| `look`, `l`, `status`, `supplies`, `threat`, `victories`, `rank` | Show current room and authoritative supplies/threat/victories/rank. |
| `go <exit>` | Travel using a runtime-validated exit; costs one supply and is rejected during an encounter. |
| `say <text>` | Bounded text; local host prints to the local actor, multiplayer uses room recipients. |
| `forage` | Restore random supplies when safe, below capacity, and no recovery is pending. |
| `encounter` | Start a random threat encounter. |
| `strike` | Spend one supply to reduce threat; final strike increments victories and schedules recovery. |
| `rest` | Schedule one supply recovery when safe and below capacity. |
| `help`, `quit` / `exit` | Executable/terminal commands; the browser displays command guidance and uses Logout. |

Ranks are Scout at 0–2 victories, Pathfinder at 3–5 and Trailblazer at 6+.
Supply capacity is nine. Forage before encountering: existing combat rules can
leave an actor with positive threat but zero supplies. This patch preserves the
balance contract; it does not add retreat/defeat mechanics or claim a balanced
production game.

## Timing and persistence boundaries

Recovery is scheduled one second after rest or a final strike. It is applied by
`runtime.advance`. The blocking terminal adapter applies it on the next command
after the deadline. Multiplayer now polls due work independently every 100 ms;
PostgreSQL mode commits timer state and audit before making it visible. Live
idle/restart/failure acceptance remains required. Issue `status`/`look` to inspect
current state; no separate browser push event is promised for recovery.
Executable and preview progress are ephemeral. Local multiplayer retains
progress across reattachment while the server lives. PostgreSQL multiplayer uses
the already implemented general gameplay snapshot/receipt transaction path;
live durable recovery/failure verification is still required.

Terminal input is bounded to 4,100 UTF-8 bytes, invalid/oversized complete lines
are drained, output escapes control/bidi characters, and EOF/quit/I/O failure
attempt attachment cleanup. Gameplay randomness uses the OS `getrandom` adapter,
not a fixed production test RNG. Tests inject deterministic randomness.

## Generated executable

Create a new project; existing generated files are not silently overwritten:

```bash
cargo run -p worldwright --locked --offline -- project new ./expedition-game \
  --name expedition-game --template expedition
cd expedition-game
cargo test --offline
cargo run --locked --offline
```

The first test command creates Cargo.lock; retain/review it. Generated main now
calls `worldwright_expedition::terminal::run` with embedded world JSON, so the
compiled game runs outside its generated directory. New projects carry a bounded
private SDK in `sdk/` with relative dependencies, so the whole project can move
before compilation. Installed CLIs can use `WORLDWRIGHT_SDK_BUNDLE` pointing to a
trusted matching bundle. Preserve its provenance manifest; registry dependencies
still need an approved cache/bootstrap, and this does not grant licensing rights. An older generated expedition main can opt in with:

```rust
fn main() -> std::process::ExitCode {
    worldwright_expedition::terminal::run(include_bytes!("../content/world.json"))
}
```

## Official artifact preview

```bash
cargo run -p worldwright --locked --offline -- content package examples/expedition/world.json \
  --output ./expedition-playable.wwr --release-id expedition-playable \
  --source-revision local --builder local --game expedition
cargo run -p worldwright --locked --offline -- preview ./expedition-playable.wwr
```

Use a new output path with an existing parent. The worker verifies the artifact
and passes its `GameContract` into preview dispatch. Reference artifacts keep
the original reference terminal behavior; expedition artifacts run the shared
playable expedition host. `worldwright play <raw-json>` remains reference play;
a raw room namespace is not used to guess a game's contract.

## Multiplayer/browser

Use the existing server startup workflow with `--game expedition`. For local
acceptance, also specify the expedition content and an existing valid Argon2
account registry; for PostgreSQL, the active artifact, seeded world and selected
server game must agree. Existing TLS/origin/authentication requirements remain.
Browser commands `forage`, `encounter`, `strike`, and `rest` map to the matching
namespaced `game_action`; status aliases map to `look`. Each new page has a UUID
request namespace to avoid unintentionally reusing durable receipts from an
older page. No cookie, account authority or counter mutation is performed in JS.

## Studio packaging

Select **Game rules → Expedition** before packaging. The optional HTTP package
field `game` accepts `reference` or `expedition`; omission/null retains the old
reference default. Unsupported names fail explicitly. This is additive for old
HTTP requests; no draft, artifact-sidecar, content, manifest or PostgreSQL schema
format changes were made. Rust callers constructing `PackageRequest` literals
must supply the new `game` field (`None` for old behavior).

Game choice is hashed into the existing manifest, and preview consumes it. The
selector only affects new packaging, not an existing immutable artifact. Old
reference artifacts are not rewritten. Publication still requires exact-artifact
preview and independently authorized review/activation.

## Regression coverage added (not yet executed here)

- Four terminal-host tests: combat/recovery, rejected action/rest/forage/movement,
  six-victory rank progression, and invalid/oversized input framing.
- Generated-project integration test now builds and executes the expedition game
  from outside its project. Nested Cargo gets an explicit private target directory.
- Real HTTP/WebSocket multiplayer fixture test exercises forage, random threat,
  strike resolution, all ranks, scheduled combat/rest recovery, and reattachment.
  It uses local storage and is NOT PostgreSQL evidence.
- Studio packaging test verifies reference default, expedition manifest identity,
  immutable readback, and unsupported names.

Run on the real toolchain before accepting the patch:

```bash
cargo fmt --all
cargo check --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo build --workspace --locked --offline
```

On Windows use the same pinned toolchain/native-target workflow used previously.
Retain the new test results separately from the historical Windows evidence.

## Checks actually performed for this patch

Read back the changed repository files through EWW. In a separate sandbox,
Rust grammar parsing passed for nine touched Rust files; TOML parsing passed for
the changed manifests and lockfile; both browser JavaScript files passed Node
syntax checks. The deterministic browser fixture passed four action mappings,
seven status aliases, unique bounded per-page request IDs and malformed command
rejection. Preview/generator/Studio dependency wiring was checked from readback.
These are NOT Rust compilation, Clippy, Rust-test, real browser or database results.
The browser contract fixture is saved as `scripts/test-expedition-browser.cjs`:

```bash
node scripts/test-expedition-browser.cjs
```

Cargo.lock only adds local-package dependency edges to the already locked
expedition/getrandom packages; registry versions/checksums were not changed.
Confirm Cargo accepts it using the locked check/test commands above.
