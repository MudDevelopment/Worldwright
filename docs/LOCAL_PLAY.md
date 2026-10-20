# Local Play v0.1

Local Play is a single-player terminal host for the existing validated JSON loader and Look/Move/Say runtime. It is not a multiplayer server, login system, save-game system, or production client.

## Run

After the normal bootstrap:

```bash
cargo run -p worldwright --locked --offline -- play examples/two-room/world.json
```

Or run the built binary directly:

```bash
worldwright play examples/two-room/world.json
worldwright play --help
```

Paths are relative to the caller's working directory. Quote paths containing spaces. Both normal and OS-native non-UTF-8 paths are supported where the OS permits them. The reserved help arguments can be disambiguated from filenames with a path prefix, e.g. `./--help`.

## Commands

| Command | Behavior |
|---|---|
| `look`, `l` | Execute Look, display addressed title/description, list the current room's exit labels |
| `go <exit-label>` | Resolve the label only in the current room, execute Move, then execute a separate automatic Look |
| `say <text>` | Execute Say; display messages addressed to this local attachment |
| `help` | Print the local command vocabulary; no world action |
| `quit`, `exit` | Stop processing input, detach the player, and drop the in-memory world |
| EOF | Same clean shutdown; a final unterminated nonempty line is processed first |
| Blank line | No action |

Commands and exit labels are case-sensitive. Direction shortcuts and quoted-string interpretation are not implemented. Spaces/tabs may separate tokens; speech itself must satisfy PlayerText (nonblank, no control characters, at most 4096 UTF-8 bytes). Leading command/separator whitespace is ignored; trailing speech spaces are preserved. CRLF and LF are accepted, with a final CR also stripped at EOF.

A starting Look happens automatically. A session starts with one local character at the JSON `entry_room`. No second character or implicit broadcast recipient is invented.

## Example session

```text
look
go north
look
say Hello from the observatory
go south
quit
```

Piped input works without terminal prompts:

```bash
printf 'look\ngo north\nsay Hello\ngo south\nquit\n' | worldwright play examples/two-room/world.json
```

The `> ` prompt is emitted and flushed only when both stdin and stdout are terminals. No terminal-library dependency or alternate-screen mode is used. Normal OS interrupt behavior remains unchanged; Ctrl-C is not intercepted for graceful cleanup.

## Exit status and errors

- **0:** Explicit quit/exit or EOF completed cleanly. Recoverable input errors do not make an otherwise clean session fail.
- **1:** Content loading, startup, runtime invariant, counter/time exhaustion, or terminal I/O failure. Runtime rejections beyond ordinary parser/unknown-label errors stop this simple adapter rather than masking an invariant failure.
- **2:** Incorrect process command-line usage.

Unknown commands, missing/extra command arguments, unknown current-room exits, invalid UTF-8, and overlong/control-bearing lines are reported on stderr without executing an action. Titles, descriptions, addressed messages, help, and shutdown output go to stdout. An I/O error may leave a successfully committed in-memory move undisplayed; it does not roll back that move.

## Resource and rendering boundaries

The input framer retains at most **4101 bytes** (4100 command bytes plus possible CR), drains excess input through newline/EOF, and rejects the entire oversized line. It does not execute an oversized line's suffix as a second command. Draining is memory-bounded, not time-bounded: local stdin can still wait indefinitely for more input. The stdin library buffer is additional bounded storage.

Player output retains complete 4096-byte fields and normal Unicode. Control characters, Unicode bidi-direction controls, and Unicode line/paragraph separators are escaped before rendering. No raw snapshot, full-world lookup, or arbitrary debug structure is printed. Error/source labels use the loader's bounded diagnostic rendering.

## Trusted-host implementation

`apps/worldwright/src/commands/play.rs` owns session setup and execution. `play_input.rs` owns bounded line framing and input parsing.

The host supplies fresh process-local world/account/character/connection identities, adds the character through WorldBuilder, registers ReferenceGame, builds WorldRuntime, and attaches through the existing trusted-host API. These fixed local IDs are not credentials or persistent identities. No user-provided ID becomes authority.

Every gameplay action uses WorldRuntime::execute. Current-room label resolution is a host-side projection of that actor's location; the runtime independently validates the destination edge. A corrupted lookup cannot create a permitted edge. The adapter filters each message by exact ActorContext, including generation, and checks that the attachment is still current before delivery.

Operations use checked nonzero counters; failed submitted actions consume their correlation IDs. Time comes from process-local Instant elapsed milliseconds and a checked conversion to Tick. Equal timestamps are allowed; no wall-clock timestamps are used. ReferenceGame does not consume randomness; the adapter returns RandomExhausted if asked instead of inventing a random generator.

Move and its automatic Look are separate executions. Failure of the latter does not undo a successful Move. Quit/EOF and I/O/runtime failures attempt detach; the original failure is preserved if detach also fails. Dropping the host discards all world state.

## Verification and scope

CLI subprocess tests cover walkthroughs, declared entry rooms, one-way travel, invalid inputs, CRLF, EOF, large/invalid input, Unicode, paths, help, and startup failures. Unit tests cover runtime state/counters, tampered lookup rejection, exact attachment delivery, cleanup on read/write failure, prompt selection, and full-size safe rendering.

See [verification](VERIFICATION.md) for actual results and platform/dependency provenance. Local Play remains a local-only terminal host; authenticated Transport v0.1 now exists separately in `worldwright-server`. Persistence is the next major implementation step.
