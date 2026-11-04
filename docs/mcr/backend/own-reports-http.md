# BE36 public USER Report and own-status vertical

Status: **IN_PROGRESS**. Source work, nine pure HTTP/contract guard regressions,
and formatting pass. Current Gateway/Concert compilation and actual HTTP/native
qualification are **NOT_RUN** pending a fresh bounded resource allocation. The
previous 23 component checks remain historical evidence on `34c3dce6`; they did
not certify the public Report routes or terminal HTTP semantics.

The base is canonical publication `0177cca15c46bb91dc01204e10e2c9f27a0345fd`,
whose second parent is the prior developer handoff `2ab64fa1`. Root's canonical
receipt establishes delivery; there is no consumer proof, review or ACK gate.
The same isolated worktree is reused on `codex/mcr-be-own-reports-http`.

## Ownership and actual defects

The assignment initially grants `backend/scripts/mcr/own_reports_http.py`,
`test_own_reports_http.py` and this document; the optional new Rust contract test
file is unnecessary. Root subsequently transfers only Report hunks in Gateway
`concert_product/feature_routes.rs`, optional necessary router glue in its
`mod.rs`, and Concert `operations_live/runtime.rs`'s CreateReport arm/minimal safe
projection helper. Existing native tests and the two Report documents are
returned for the affected regressions. No central manifest, proto, SQL, helper,
unrelated routing or frontend edit is authorized or made.

Two concrete static defects are corrected:

- Gateway did not mount `POST reports` or `GET me/reports`. The existing feature
  router now admits those exact methods, preserves the original signed request,
  applies the existing canonical access/origin/browser-intent/body framing and
  no-store error boundary, and bypasses the Gateway journal only for the keyed
  Report mutation. The body bound is 64 KiB. The own query remains closed to
  canonical `limit`, `state` and opaque `cursor`; no foreign SELF selector exists.
- The Concert producer always returned a receipt with HTTP 202, including a
  committed SUCCEEDED result. The frozen C04 contract reserves 202 for PENDING
  and defines terminal success as 200 `admin.Report`. The Report branch now
  reads the actual receipt-bound actor/event/resource/revision under its retained
  canonical scope, returns only the safe Report fields, and maps actual failed
  receipts to their documented terminal errors. Report quota uses RATE_LIMITED.
  No SUCCEEDED receipt is disguised as pending and no Report is synthesized
  from request data. Receipt/outbox/native ownership and completion stay intact.

The nine native regressions now expect the frozen 200 safe projection and
404 duplicate terminal, retaining actual database receipt checks, revocation,
quota/replay/conflict and rollback evidence. The original receipt must remain
SUCCEEDED with result status 200 and the same real Report UUID.

## Harness and provenance

The newly authored driver calls the existing `in03_stack_runtime` launcher and
`world_in01_prepare` real-account wrappers without editing or copying their
implementation. These helpers own seven actual services, strict managed mTLS,
private mail delivery, signup/verify/login, canonical C03, exact source-derived
migration manifests, bounded processes and teardown. The Report driver adds
independent ephemeral cursor/receipt keys and USER mode using the same canonical
core and delivery composition. No operator, verified-age, reviewed publication,
target/media authority or positive identity fixture is fabricated. The built-in
USER/no-media SafetyInteraction and actual target event admission already suffice.

The original helpers remain byte for byte. Source/SQL/tool/binary hashes are to
be frozen outside the target before native startup. The full C03 body and bearer
remain in memory; neither is retained or hashed. Credentials remain in the
existing owned private fixture directory and disappear at teardown. Diagnostics
contain fixed labels/statuses and safe business/ledger digests.

Root authorizes reuse of the actual five Auth/Friend/Profile/Post/Notification
mains at `18076c951a7283354c869caaf64d40491ca52d75`, from
`/private/tmp/mcr-i03-yv3aumx1/inputs.json`, with build/source/SQL provenance in
`/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-root-functional-cohort-__bza8gy/qualification.json`.
Their explicit source-specific migration manifests are used. They are foundation
evidence, not evidence of current Report code. Only changed Gateway and Concert
mains are rebuilt, plus necessary affected test executables. No broad seven-main
rebuild, new checkout, target or bulk archive is needed.

## Required functional qualification

Run the driver with the existing strict interpreter:
`/private/tmp/mcr-contracts-admission/bin/python -B`. Every schema-declared format
checker must be present before native work; response checking uses the unchanged
full `validate-domain.Contract.validate_http` oracle, including 202/PENDING and
receipt actor/event/key/operation/status semantics. The nine pure guard tests
passed with `-B -W error -m unittest discover -s backend/scripts/mcr -p test_own_reports_http.py`.
They are parser/transport checks, not successful canonical authority evidence.

The bounded native journey must establish:

- Three distinct real Gateway signup, mail verification, login, Profile and C03
  principals, followed by actual A→B, A→C and B→C USER/no-media reports.
- Exact public replay, changed-body conflict, correlated original receipt/status,
  safe private own list/pagination, cross-actor/filter/limit cursor denial and
  rejection of a foreign reporter selector. Reads/replays/conflicts create no
  additional report, receipt, outbox or audit command effects.
- A real PostgreSQL restart preserves business rows, original receipts/outbox
  and all seven actual migration ledgers; original public status/replay remain
  correlated after restart.
- An actual HTTP read retains canonical Auth exclusion: a temporary owned table
  lock pauses that read, an actual Auth suspension writer demonstrably waits via
  `pg_blocking_pids`, the original read finishes, and the next HTTP read is denied.
  The lock creates no identity or positive role. All temporary SQL sessions close.
- Actual prepared transaction count is zero before teardown; all owned service,
  Redis and PostgreSQL PIDs, listeners and the exact private root are absent.

Actual commands, allocated caps/clock, immutable input pins, failures, exits,
resource release and canonical committed publication will be appended after the
developer completes the relevant checks and fixes. The aligned historical goal
remains truthfully unfinished with its existing BLOCKED API metadata; no new or
falsely completed goal is created. Public Report integration is open until this
journey passes. Moderator/media evidence, BE38 reviewed operations, IN03 and whole
BE36/BE38 acceptance remain open. Performance qualification is deferred.
