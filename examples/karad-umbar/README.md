# Karad-Umbar: The Last Muster

A text MUD set in a dwarven kingdom sealed beneath a mountain. Start in the muster hall. `go east` and `look` reveal the two missing witnesses. Find their directions, compare notes where your characters can hear each other, and follow the two seals to the Hearth Roll. The district is playable alone; in multiplayer, scouts can split the search. `go gate` from muster opens the entire 100-room kingdom immediately. At the gatehouse, `go onward` or `go backward` follows the Ring Road; in the other districts `go in` leaves it. Use `look` for the actual exit labels.

## Run from the Worldwright root

```powershell
node .\examples\karad-umbar\test.cjs
$ww = '.\.worldwright-local\target\x86_64-pc-windows-msvc\debug\worldwright.exe'
$world = '.\examples\karad-umbar\world.json'
& $ww content validate $world
if ($LASTEXITCODE -eq 0) { & $ww play $world }
```

Use the full path to the built CLI if `worldwright` is not on PATH. `play` is single-player and ephemeral. For a different shared commission, compile an edition (existing output refused):

```powershell
node .\examples\karad-umbar\generate.cjs winter-watch .\.worldwright-local\temp\winter-watch.json
& $ww content validate .\.worldwright-local\temp\winter-watch.json
if ($LASTEXITCODE -eq 0) { & $ww play .\.worldwright-local\temp\winter-watch.json }
```

## Persistent multiplayer acceptance

Use two private test accounts and a fresh disposable PostgreSQL world. Package the chosen JSON as a **reference** artifact; seed, review and activate that exact artifact using the existing release workflow (`docs/RELEASE_V1.md`, `docs/SERVER.md`). Start the server in database mode with the active digest and a single writer. `serve --content` is not a persistence test. Do not point at an old R2 evidence cluster.

In two separate browser profiles, meet at muster and verify room-local speech. Split into different districts and confirm distant speech does not arrive. Read the two witness marks and reunite to exchange directions. Leave one character at Second Witness Seal and the other at the slate. Stop and restart the owner on the same DB/artifact; reattach and verify both distinct positions and no historical speech rebroadcast. Finish the commission. Record artifact, alias, positions, result and cleanup; do not record secrets. This is an acceptance recipe, not a claim of an observed pass.
