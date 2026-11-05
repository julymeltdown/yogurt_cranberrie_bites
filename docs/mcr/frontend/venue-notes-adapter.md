# Venue Notes adapter — N-ADAPTER-01

Status: ACCEPTED within the bounded headless adapter/source qualification scope. Unbudgeted assigned objective: implement the actual raw N
client to the existing primary VenueCapabilities Notes partial, meaningful
focused qualification and fixes, provenance, scoped committed direct412e handoff
and publication fixes. The incompatible historical QA01/C04 goal is usageLimited;
it remains unchanged. No manager review/ACK stage applies.

Exclusive retained checkout: /Users/lhs/Desktop/mcr-worktrees/sl-world, branch
codex/mcr-fe-PI10-parade-ink-public-auth, base b977c447fc89adda653b7a823f67d1771fd8f219.
PI10 publication8b7ba097 and old resource FULL are preserved. Root153's literal
single-writer grant follows40fde's STOP/RETURN of zero unwritten N-factory code.
Only frontend/src/app/venue-notes-adapter/**, this document and
 docs/mcr/evidence/venue-notes-adapter/** are writable. Existing raw N, generated
contracts, primary ports, Map/L/helpers, aggregate composition and Root seams are
read-only. No new worktree, dependency installation or sparse expansion.

Borrowed ports: e9535129cffa3917f5b9b5ae3f5ee992609c4419 and identical647c18e,
frontend/src/features/paradise-city-map/ports.ts SHA256
94caaab2f25b5c7546a6b5b278d47b4dcfca51ac31d1227efa1147d817bdf328.
Raw client:336375622af28116904b8b923651cad00b60806e, based on sourcea378329c,
Notes OpenAPI SHA5b14c48c7b288ba4a1c7a68c5e0d84c16cc9805ae87e98b18c091957c2f24f1e.
Root reports40retained+21canonicalHTTP cases/types/lint/generator qualified;
this adapter requires its own focused execution and makes no live backend claim.

Completion criteria: exact audience and server expiry mapping, inherited reply
parent/revision/audience/expiry, full five-field privacy and original signal,
same borrowed qualified clock, explicit original UUID, no uncertain-write replay,
status correlation and rejection of substituted receipt/resource/operation,
unique reviewed physical tuple-to-ART lookup, synchronous withdrawal scrubbing,
immutable partial read/subscribe/rights and unavailable missing inputs. Tests
must execute actual raw client/decoder plus the adapter; qualify relevant strict
TypeScript and lint, no unrelated matrix replay. Render/browser checks only if
needed by changed behavior; this leaf adds no rendering.

The actual six-operation N client has no status or create-rights producer.
Backend owner213 confirms proposed authenticated original-key C04 routing returns
NoteCommand, not CommonCommandReceipt, and mounted NEW N status routing remains
unqualified. The adapter borrows a nullable status producer of that existing N
type; it invents no endpoint or response identity fields. Root retains actual
routing and new rights. Missing status stays UNKNOWN without replay. Missing
creation rights/day-end policy stays unavailable; per-note can_reply cannot grant
creation. END_OF_DAY maps to VENUE_END_DAY, never a local midnight calculation.

Resource preparation: VENU-NOTES-ADAPTER-PREP32-0255 admits32MiB total positive
growth through04:00UTC for owned source/helpers/readset/docs/Git only. No Node,
tests, browser, target or borrowed-source materialization under this book. All
prior PI10 maxima/history remain charged; execution needs a fresh finite offer.

## Primary-port consumption

`createVenueNotesAdapter` is a headless leaf. It exposes the existing primary
`postNote`, `reply` and `resolve` command signatures, and an immutable
`VenueNotesSnapshot` built from `CapabilitySnapshot` fields. The aggregate owner
merges that partial with L and routes retained keys using the read-only
`ownsCommand` predicate. The leaf does not install a second controller, router,
auth client, socket, clock, poller or renderer.

The owner explicitly invokes `load(typedArtLandmarkId, originalSignal)` and, for
an already visible root, `loadThread(noteId, originalSignal)`. `read()` and
`subscribe()` never acquire or send. A successful command receipt is not an
optimistic note: the owner refreshes through these read methods. Lists expose
only the decoded current page; `next_before` is not claimed as complete history.
A new selected place drops the old projection. Thread replies are accepted only
through the actual raw decoder's parent/audience/absolute-expiry checks.

Required injected providers are the existing `VenueAuthority`, `VenueView` and
`MeetingClockPort`, actual `createNotesClient` result or null, a reviewed
association read port, nullable creation rights, nullable status producer and
explicit classification. Association entries bind a typed presentation ID to an
actual `NoteLandmarkSelection` tuple. They require current full scope, same site
identity, access lease and uniqueness on both sides. Neither geometry, display
labels, ID casts nor string-to-revision conversion creates physical authority.

Creation rights reuse the primary `postNote`, `noteAudiences` and `noteExpiries`
fields and require their own current scope/site/access lease. Root must supply
these from qualified actual event/friend/policy sources. `END_OF_DAY` admission
means the provider has reviewed the venue timezone and server absolute cutoff;
this adapter neither calculates nor extends that cutoff. If unavailable, omit
that option while independently retaining authorized `ONE_HOUR`. A visible
note's `can_reply` or `can_withdraw` never creates post rights. `EVERYONE` retains
the backend's eligible authenticated same-event audience; it is not Internet
public access. The server remains the authority for friendship, blocks, event
write state, capacity, inventory and idempotency.

The borrowed clock's qualified current upper server-time bound validates both
response leases and absolute note expiry. Response `server_now` is checked
against that clock and never becomes a new time anchor. Getter freshness checks
also fail closed if a source fails to emit a notification. Authority withdrawal
or abort clears notes, rights projection, thread cache and retained commands
before notifying consumers or running disposal callbacks. Current provider
replacement clears the projection and fences late requests. Read expiry does
not manufacture command failure or permission to repeat an uncertain write.

Every write requires the caller's original valid UUID. The bounded 64-entry
ledger does not evict uncertain keys. Duplicate keys do not send. Transport loss,
`PENDING`, `RETIRED`, missing status or invalid receipts remain `unknown`.
`resolve` requires the retained exact key, full scope, same site, original live
signal and current reviewed provider tuple. It invokes only the injected status
producer with frozen original operation/key/scope/physical tuple/parent and
first-known receipt. The actual N schema and operation are checked; known
receipt identity, timestamps and resource cannot be substituted. Actual terminal
success/failure is retained. No receipt field is fabricated to echo actor, event
or key. Authenticated same-context status correlation and backend reauthorization
are the injected producer's obligation; actual Root routing is still a separate
undelivered/unqualified seam.

Imports are type-only across the existing frontend ports and runtime-only into
the existing spatial identity utility and raw N client/schema. No backend module,
server configuration, database access or secret is imported. No dependency or
lockfile change is needed. Private qualification materializes the exact borrowed
readset outside the checkout and includes transitive types for strict checking;
it does not grant ownership of those source files or qualify their complete app.

## Qualification record

SOURCE01 at `06fc56518bd36ff3e7d2fb81db6ae2f7ffdf1678` executed31 meaningful
actual-client/decoder cases:29 passed and2 status-substitution oracles failed.
The fixture supplied HTTP202 for create operations, but the actual frozen
contract accepts201. The raw decoder correctly refused202, so there was no
first-known receipt to compare with the later status response. Repair uses201
and asserts the retained receipt explicitly. No production receipt fence was
removed to make this pass.

Strict types also exposed Vitest's generic mock widening and an obsolete
borrowed HTTP/registry readset from the earlier Map checkpoint. The fixture now
preserves the actual generic method signature with separate context capture;
read-only canonical HTTP/domain/registry inputs come from published
`51514ca4074fb131d7fe55577c1766935d71ce60`. The raw N source remains33637562
unchanged. Two listener snapshot copies are now expressed with `Array.from`,
preserving notification isolation while satisfying lint.

SOURCE01 is a retained FAIL, not a qualified release. It reached literal FULL
with zero resources before diagnostics were inspected. Its actual guards and
command logs remain in `../evidence/venue-notes-adapter/source01/`.

SOURCE02 at `37aabce2c0b25f2010b0ebe43d8711b4ad29871c`, readset
`adfca7804cf99b45bd412154fd197a1d11bd6e6b43db9ae8b0947c8d0b8c214a`,
passed all12 changed create/status/withdrawal-fence cases (19 unchanged cases
skipped) and the full relevant strict TypeScript closure, both exit0. The
independent guard then stopped the round before lint: its cumulative positive
high-water668,398,336B exceeded the conservative668,267,264B limit by131,072B.
This is retained as an overall FAIL; completed case/type results remain factual.
The higher guard MAX, entire32MiB preparation charge, repeated guard/control
reserve charges and all prior PI10 history remain accounted, without refunds.
Literal FULL1793848380.490931 released all owned resources before inspection.
Only the final changed-path lint remains to run; no passing test/type replay is
needed for that step.

A further responsible-developer boundary check found that a repeated original
key was checked after new-write admission. If its read lease, rights, provider
or original signal had since withdrawn, a retry could return `rejected` despite
the original outcome still being unknown. The key-ownership fence now precedes
new-write admission in both commands. Four focused regressions cover those
withdrawals without sending or inventing failure. This changes no API or source
ownership; only these new cases, affected strict types and final lint need run.

SOURCE05 used the changed-input7742cc361 candidate and started its own fixed
300/360-second clock. Both independent initial live samples completed. It stopped
before any Node command on original PID76856, birth `Thu Nov 5 12:26:53 2026`,
whose relative Map preparation invocation did not match a published absolute
script identity. The original observed identity remains in the raw evidence;
process disappearance is not a release or a retrospective waiver. The actual
Map implementation owner received the factual producer-metadata correction.
No new test/type/lint result is claimed for this round.

Literal FULL1793849215.348382 released all resources before inspection. Higher
independent WHOLE5,592,001,307B and outer cumulative NEW773,407,488B remain
charged. The final resource census also exceeded the per-family conservative
upper bound; the prior aggregate was not reset. Unused unchanged-input lint-only
03/04 offers were superseded before acceptance, not failed or executed. The
four new cases, affected types and final lint still require their own admitted
continuation. All failed source copies and logs are preserved.

## Final bounded disposition

SOURCE07 qualified the exact `7742cc3617f11905a2af62e42da35e743967aedb`
implementation/test postimages, readset
`3d5afce37be27e8947017a7ccfe25d172e0ffce06408db609ea44f7e508a7aef`.
The four newly required uncertainty-withdrawal cases passed, with31 prior cases
skipped. Affected strict TypeScript and final changed-path oxlint both exited0.
Both independent resource guards passed. Earlier SOURCE01's29 passes and
SOURCE02's12 corrected-input passes remain evidence at their recorded source
pins; this is not a claim of one fresh35-case run or a fresh whole application.
The immutable347-file5,406,858-byte readset includes actual raw N/client/decoder
and the relevant frontend public types. No live HTTP/server/browser is used by
these contract tests.

Actual SOURCE07 ACCEPT1793849828.945473 fixed WORK1793850128.945473 and
FULL1793850188.945473. Exact predecessor PI08 native01 FULL1793849747.015669
was observed and its immutable receipt SHA checked before acceptance; no chat
acknowledgment or completion barrier was invented. Independent complete LIVE
samples preceded payload. Literal PASS FULL1793849853.891082 released all
resources before inspection. Peak combined NEW821,510,912B is within the final
1,070,920,448B source-family limit and1GiB aggregate; WHOLE5,595,003,675B,
source-run own RSS up to1,132,101,632B, minimum free23,360,299,008B. All higher
old PI10/source histories, failed inputs, entire PREP32 and booked reserves are
retained without reset/refund. Unused repair08 is yielded. No further runtime
is required for this leaf.

The scope audit and exact-path manifest are the committed direct412e delivery
packet. The publisher must preserve concurrent canonical work, reconcile current
history, deliver only these new leaves, verify postimages/ancestry and return its
actual receipt. The implementation owner verifies that receipt independently.
Existing ports, aggregate composition, raw modules, manifests, lockfiles,
routers, artwork and `docs/source/` are unchanged by this assignment. The old
QA01/C04 usageLimited goal remains unchanged; the new assigned objective is
recorded here without a token budget or model/reasoning override.

The headless adapter is ACCEPTED within this assigned source/contract scope.
Production Root status admission/mount, qualified policy and reviewed association
providers, consumer/aggregate integration, combined latest-master behavior,
actual backend/public HTTP/native/multiuser/load and release remain OPEN/NOT_RUN.
Missing providers fail closed as specified above; no successful live delivery,
physical arrival, new-write permission or uncertain-command replay is invented.
