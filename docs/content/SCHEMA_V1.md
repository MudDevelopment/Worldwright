# Content schema v1

Status: first implemented, unpublished JSON content contract. Package version remains 0.1.0-dev.0. This is descriptive content, not a save game, account store, published release artifact, or live activation protocol.

## Document

All fields are required unless a default is stated. Unknown fields and duplicate JSON object keys (including escaped-equivalent keys) are errors at every level. Input is UTF-8 JSON, with no comments, includes, external references, or trailing data.

| Field | Type | Meaning |
|---|---|---|
| schema_version | integer | Exactly 1; no inference for missing versions |
| namespace | string | Lowercase segment: `[a-z][a-z0-9_-]*`; must match every room ID |
| entry_room | string | Existing room definition; advisory host entry point, not player authority |
| rooms | array | 1..1024 room definitions |

## Room

| Field | Type | Meaning |
|---|---|---|
| id | string | Core DefinitionId syntax, e.g. `reference:rooms.atrium`; at most 192 UTF-8 bytes |
| title | string | Core PlayerText: nonblank, no control characters, at most 4096 UTF-8 bytes |
| description | string | Same PlayerText contract; single-line, not HTML |
| exits | object | 0..64 label-to-room-definition references |
| allow_unreachable | boolean | Optional; false by default. True explicitly permits this room to be unreachable from entry_room |

Exit labels are 1..64 ASCII bytes matching `[a-z][a-z0-9_-]*`. Targets must be local definitions. Self-exits, duplicate labels and repeated destinations from one room are errors. Edges are directed; the author declares the reverse edge explicitly when needed. One-way travel is permitted. Reachability follows outgoing edges from entry_room; declarations exempt only their own rooms, not every downstream room. Redundant allow_unreachable declarations are accepted.

No accounts, characters, connections, rule registration, world-instance IDs, attachment generations, inventory, or runtime revisions belong in the file.

## Resource contract

Default and maximum loader limits: 32 MiB input, 32 levels of object/array nesting (root container is level 1), 100 diagnostics. Callers may lower these limits; larger or zero values are rejected. An additional fixed parser budget allows at most 200,000 JSON values and 1024 entries per container. These guards apply before typed schema deserialization, including unknown/future-version data. Room and exit collections are also bounded during typed parsing. Syntax/shape errors stop that phase; independent semantic errors are collected in room source order (exit labels lexicographically) up to the diagnostic cap, with an explicit truncation flag. Limits are not measured throughput or an allocation-failure guarantee.

The file reader reads at most limit+1 bytes and rejects non-regular files. Metadata is not used as the sole size check. Files must be trusted local paths selected by the caller; this is not a filesystem sandbox. No file is rewritten.

## Validation and instantiation

1. Bound bytes, nesting, value count and container sizes; reject duplicate object keys.
2. Read schema_version independently of v1 fields; reject unsupported versions explicitly.
3. Deserialize strict v1 shape; report line/column and a field path when available.
4. Validate definitions/text, namespace, uniqueness, entry reference, exits, and directed reachability.
5. Return immutable ValidatedWorldContent, never partially validated data.
6. Instantiate with a host-supplied WorldId: sort definitions lexicographically, assign RoomId values 1..N, add all rooms, then directed edges through WorldBuilder. Preserve labels and definition-to-instance mapping in WorldInstantiation.

Mapping is stable for the same definition set regardless of JSON ordering, NOT stable across additions/removals. It is not a persistence identity scheme. Conversion errors return no partial builder. Hosts then add characters, register rules, build the runtime, and attach independently authorized actors. Entry selection and lookup maps do not authorize player queries.

## Diagnostics and CLI

`worldwright content validate <path>` validates content and preflights builder construction using a throwaway world identity, with no characters, rules execution, or connections. Exit 0: success; 1: I/O/content/conversion failure; 2: usage error. Success summaries go to stdout, errors to stderr. No interactive game or listener starts.

Reports carry severity (currently Error), a stable code, source label, and either JSON pointer or parser path/line/column. Parser paths are Serde paths, not claimed to be JSON pointers. Semantic locations are JSON pointers with escaped tokens. Semantic source line spans are not retained. Display bounds each diagnostic field and escapes non-ASCII/control characters; no raw source excerpts are printed.

Codes: WWC_IO, WWC_LIMIT, WWC_JSON, WWC_DUPLICATE_KEY, WWC_VERSION, WWC_SCHEMA, WWC_ID, WWC_NAMESPACE, WWC_TEXT, WWC_DUPLICATE_ROOM, WWC_UNKNOWN_ROOM, WWC_EXIT_LABEL, WWC_SELF_EXIT, WWC_DUPLICATE_EXIT, WWC_UNREACHABLE, WWC_BUILD. The report exposes a truncation flag rather than falsely claiming completeness.

## Compatibility

This is the first implemented content schema. Earlier DESIGN.md examples were illustrative and did not define accepted files. There is no legacy migration to invent. Missing versions require the author to declare schema_version 1 after reviewing this contract; unsupported versions fail with a supported-version message. Future breaking schema changes must include a tested migration or an actionable safe refusal. Removing ContentPipelineStub is an unpublished Rust source API break; use load_file/load_bytes and instantiate instead.

## Example and verification

See [two-room example](../../examples/two-room/world.json) and [usage](../../examples/two-room/README.md). Tests cover strict input, resource boundaries, graph validation, deterministic mappings, CLI behavior, and real loaded-world execution. See [verification](../VERIFICATION.md) for observed results and platform limits.
