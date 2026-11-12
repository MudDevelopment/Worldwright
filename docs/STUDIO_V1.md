# Studio v1

Worldwright Studio is a local loopback browser app for editing schema-v1
drafts. It is not a database client, collaboration service, arbitrary
filesystem browser, or production deployment console.

## Start

```text
worldwright-studio serve --workspace <path>
worldwright-studio serve --workspace <path> --content <path> --bind 127.0.0.1:0
worldwright-studio serve --workspace <path> --preview-command <path>
worldwright-studio serve --workspace <path> \
  --release-endpoint http://127.0.0.1:PORT \
  --operator-token-env WORLDWRIGHT_OPERATOR_TOKEN
```

The endpoint requires exactly one token source: `--operator-token-env NAME` or
`--operator-token-file ABSOLUTE_PRIVATE_FILE`, never both. The token is read
server-side and is never sent to the browser, persisted, or logged. The
listener binds exactly IPv4 `127.0.0.1`; port `0` selects an ephemeral port.

## Workspace

```text
<root>/.worldwright-studio
<root>/.worldwright-studio-session  # operational serve sentinel; crash recovery is manual
<root>/revisions/00000000000000000001.json
<root>/artifacts/<sha256>.wwr
<root>/artifacts/<sha256>.json
<root>/preview-receipts/<sha256>.json
<root>/publication-records/<request-id>.json
```

The marker and revisions are append-only. Revisions are canonical strict JSON,
bounded to 10,000, and saved with create-new semantics. Import is read-only
until the user explicitly saves. Artifacts, receipts, and publication records
are immutable or safely replaced according to their fixed identity.

## Workflow

`Save -> Validate -> Package -> Preview -> Request`.

Validation always targets a saved revision. The package is bound to the exact
saved revision and SHA-256 digest. Preview is pinned to an artifact and reports
stale state when the draft advances. Requesting publication sends the exact
artifact bytes plus a review context; it does not approve or activate anything.

## API

| Route | Purpose | Request-body limit |
|---|---|---:|
| `GET /api/v1/workspace` | Current saved draft snapshot | - |
| `PUT /api/v1/draft` | Append a revision | max input |
| `POST /api/v1/import` | Inspect draft without saving | max input |
| `POST /api/v1/revisions/<revision>/validate` | Validate saved revision | 4 KiB |
| `GET /api/v1/revisions/<revision>/export` | Download canonical JSON | - |
| `POST /api/v1/package` | Create/reuse immutable artifact | 4 KiB |
| `GET /api/v1/artifacts` | List bounded artifacts | - |
| `GET/POST /api/v1/preview...` | Preview lifecycle and input | 4 KiB |
| `GET/POST /api/v1/publications...` | Publication request/status | 4 KiB |

Every request requires exact `Host`. Mutations require exact same `Origin` and
JSON endpoints require their exact media type. Responses include CSP,
`nosniff`, `no-store`, and `no-referrer` headers. No CORS headers are emitted.

## Security and recovery

Studio confines HTTP filesystem work to its fixed workspace layout and rejects
symlinks, unexpected filenames, noncanonical revisions, mismatched digests,
orphaned artifact records, and oversized files. Artifacts and preview receipts
are create-new; publication refresh uses validated `.next`/`.previous` replacement
journals with explicit recovery. It does not silently roll back an uncertain
remote operation.
Preview is direct/no-shell,
environment-cleared, output-bounded, and uses the existing M5 watchdog; it is
not an OS sandbox or network firewall. Publication authority and audit remain
in the authenticated M6 server/PostgreSQL path.

Delete-room is local-only, blocks incoming references and entry-room removal,
requires typing the stable ID, and offers cancel/reload recovery. Save
conflicts preserve local edits and never overwrite the newer revision.

## Accessibility and evidence boundary

The UI includes semantic forms/buttons, labels, skip navigation, live status
announcements, visible focus, responsive reflow, reduced-motion styling, and
dialog confirmation flows. No manual screen-reader, keyboard, zoom, or browser
matrix was performed; accessibility conformance is not claimed.

## Game selection update — November 9, 2026

Studio now offers Reference/Expedition in the package form and sends optional
`game` on the package request. Missing/null retains the reference default;
unknown names return an explicit unsupported-game error. Existing draft,
artifact-sidecar and publication schemas are unchanged. Artifact game identity
remains hashed into its manifest, and official preview now dispatches that
contract rather than always choosing reference rules. Current regression
execution is pending; see `EXPEDITION_PLAYABLE.md`.

## R5 workflow/recovery increment

See `R5_ACCEPTANCE.md` for the current API table and acceptance boundaries.
Studio now guards asynchronous draft/validation/preview/publication responses,
shows verified artifact game identity, bounds UI request waits/responses, and
provides local publication replacement recovery. Drafts/revisions/sidecars keep
their existing formats; packaging choice is not a persisted game preference.

Publication may alternatively use `--operator-token-file ABSOLUTE_PRIVATE_FILE`
with the loopback endpoint (mutually exclusive with token-env). Each explicit
operation reads the file afresh; browser code never receives the bearer or path.
The environment option remains startup-only. Expiry/denial is not automatically
retried. Use the connection/recovery details panel for nonsecret guidance and
explicit `.next`/`.previous` reconciliation; it never approves or activates.
For a lost initial submission response, obtain the exact request ID through an
authorized operator and use **Reconcile existing request**. This only reads status
and restores matching local history; missing/corrupt/mismatched artifact or
original preview evidence fails closed. Unknown IDs cannot be discovered here.

Updated serve processes refuse a second writer for the same workspace. After a
crash, never delete `.worldwright-studio-session` merely because its PID looks
absent or old. Establish all writers stopped, preserve a private workspace copy,
and follow [R5 recovery instructions](R5_ACCEPTANCE.md). The permanent workspace
marker and journal evidence must remain intact. Direct/older writers do not
participate in this cooperative guard.

The complete [check plan](R5_CHECK_PLAN.md) covers focused/full native tests,
SDK relocation, fixtures, real backend/manual recovery and independent acceptance.
The [ledger](verification/r5-implementation-ledger.md) separates executed local
fixtures from unrun native/live gates. R1/R2/R4/R5 acceptance remains open.
