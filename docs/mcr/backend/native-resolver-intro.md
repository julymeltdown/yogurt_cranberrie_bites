# Intro native resolver integration

Status: INTEGRATED. [Fresh combined native qualification](root-intro-union-mural-dfc63137.json)
passed at `dfc63137` (code `6cae9e53`). This does not activate a Product route or
complete IN03.

`config/native_resolver.rs` selects the existing Intro native witness adapter for
exactly the six operations accepted by `intro_live/protocol.rs`. Parent RPC,
operation, event, method and target must match. List queries also pass the actual
closed Intro query validator. Wrong method, foreign operation, unsupported or
duplicate query keys, invalid limit, bad encoding and noncanonical detail IDs
cannot select the committed witness adapter. The selected adapter still checks
the exact original transaction and durable witness; UNKNOWN/error never tries a
second adapter. All concrete adapters use the same configured pool and instance.

The existing Intro canonical fixture now uses the actual union for its
Auth/Friend-authenticated settlement service and exact native-witness case. Its
Concert store uses `mcr_concert_intro`, within the unchanged dedicated-store
policy. No production pool admission is widened. All seven native tests passed through the union and retain
real canonical fencing, publication/privacy behavior and expected UNKNOWN
quarantine. The native-witness case checks committed witnesses, foreign instance,
wrong witness, in-progress, aborted and unwitnessed committed SQL. The existing
union route table includes all six Intro routes and cross-domain relabeling.

Root retains the resolver union. CF02 owns only canonical and Participant factory
files in its isolated worktree. Production mounting, shared runtime supervision,
real HTTP and the complete IN03 journey remain open.

The existing union controller passed its three route/constructor tests and actual
canonical native matrix. Intro also passed two pure and four retained engine
cases plus an actual PostgreSQL restart. Both Cargo phases exited zero; the
target released before any copied execution. All owned roots were removed, and
the whole run completed in 57.581 seconds below the stricter 1 GiB mural cap.
