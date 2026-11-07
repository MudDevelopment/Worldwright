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

The endpoint and token options must be supplied together. The token is read
server-side and is never sent to the browser, persisted, or logged. The
listener binds exactly IPv4 `127.0.0.1`; port `0` selects an ephemeral port.

## Workspace

```text
<root>/.worldwright-studio
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
are create-new; publication refresh uses a `.previous` rollback sequence.
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
