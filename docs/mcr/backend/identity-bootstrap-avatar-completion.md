# Original BE04, BE06 and BE07 completion

Status: **ACCEPTED for original BE04, BE06 and BE07 criteria**. This is the
responsible developer's implementation record, not application-wide release acceptance.

## Durable objective and custody

Finish original BE04 canonical identity/private projection, BE06 idempotent
bootstrap/defaults/capabilities/manifests/clock, and BE07 validated avatar/theme
persistence with optimistic concurrency. Reuse existing source and qualified
native/product evidence; implement genuine remaining behavior, perform relevant
developer-owned validation and fixes, document provenance and limits, and deliver
a committed handoff directly through the current named publisher412e
(`mcr(delivery) Master publication`, `01a1a5d9-332b-7822-a8f7-412e295fa211`).
This publication routing supersedes the earlier Root1fc handoff destination. Each original row
receives its own truthful final disposition. Shared production composition and
integration/device gates retain their actual owners. No manager review or ACK
is a prerequisite to implementation, validation or publication.

The source base is `64947bbfb0b63c26856dbb65bb3244d07ae21523`, on isolated branch
`codex/mcr-be-BE04-BE06-BE07-completion` in the retained exclusive
`be-world-projection-context` checkout. QA10 candidate and evidence are preserved
on `codex/mcr-qa-QA10-candidate-report` at
`21dabbc41de09371fec03be4705c89b8d0a7e091`; its publication is independent.
Root explicitly stopped the participant live update/coordinator/service/profile
leaves, bootstrap participant/default projection hunks, and actual avatar
repository for this transfer. There is no repository `participant.rs`; no such
path is invented. The existing bootstrap/avatar/participant producer tests and
needed dedicated helpers plus this lane document are granted.

The persistent goal API still reports the historical BE-PROFILE-SELF goal as
BLOCKED. It is incompatible with this whole-three-task objective and the API
cannot create a second unfinished goal. Latest explicit continuation authorizes
this durable objective while preserving that historical goal truthfully. No
reset, false completion, fake ACTIVE state or token budget is used.

Root retains main/startup/World runtime, canonical configuration, shared contracts,
CORE, generated artifacts and manifests. Release/schema/spatial code remains
its incumbent writer's scope. Profile SELF remains unchanged; owner232 retains
its current leaves. U03 optional ticket/audience semantics await the already
pending human decision; no additional field, audience or default is invented.

## Original criteria and reused evidence

| Task | Original behavior / acceptance | Existing source and actual evidence | Current disposition |
| --- | --- | --- | --- |
| BE04 | Canonical Auth UUID, basic Profile, suspension and explicit provider binding; email-only participation, foreign claims denied, private fields excluded | Application identity resolver and service adapter; Auth owner native22 cases at `c300a231`; guarded owner/Participant native join `6767ee8e`; current participant/public-profile allowlists and subject binding; accepted two-account IN01 cohort | ACCEPTED for original BE04 criteria from source and retained actual owner/public-reader/IN01 evidence. No new provider/X prerequisite or code change required. |
| BE06 | Idempotent participant/default preset, capabilities, manifests, server clock; one participant under repetition, truthful optional degradation | Bootstrap repository and release binder; native16 concurrent calls, suspension/rollback/deadline/restart; selected six-preset binding; bootstrap feature consumer tests and accepted IN01 | ACCEPTED: canonical deletion exclusion is fixed; the fresh real-PG concurrent/bootstrap, physical-restart persistence and independent deletion-fence cases all passed at helper source `de93c210d`, using unchanged compiled Rust source `1289d90e9`. |
| BE07 | Asset IDs/layers/version and bounded payloads, avatar/theme CAS; invalid URL/item rejected, stale conflict, other-user isolation | Domain catalog, production avatar repository and participant live coordinator; native legacy/selected witness suite, reviewed motion v2 native CAS/restart, native owner SELF join and accepted IN01 customization | ACCEPTED for original BE07 criteria from unchanged validation/CAS and retained native layered/selected/reviewed-motion plus actual owner save/reload evidence. No artwork or contract change required. |

Original criteria come from `generated/TASKS_EN.json` and specification BE04/06/07,
not the stale NOT_STARTED planning status. Prior owner records remain intact:

- `docs/mcr/evidence/auth-identity-owner-integration.md` and
  `docs/mcr/evidence/auth-identity-owner/native.json`: real Auth ownership,
  foreign provider/row binding, deadline and revocation semantics.
- `docs/mcr/backend/bootstrap.md`: actual native uniqueness, guarded response,
  suspension, optional capabilities, persisted state and restart evidence.
- `docs/mcr/backend/avatar-persistence.md`: native CAS/isolation, invalid inventory,
  strict selected pack, exact transactional witnesses and later joined completion.
- `docs/mcr/contracts/avatar-motion-v2/qualification.md` and retained qualification
  receipts:19 normal and6 native tests, reviewed catalog/config/resources,
  twelve-way CAS, preserved legacy rows, production constructor and restart.
- `docs/mcr/backend/participant-live-evidence/joined-native-6767ee8e.json`:
  actual Auth/Friend/Profile/Participant prepared commit, save/reload and abort.
- `docs/mcr/evidence/in01-world-browser/attempt-11/report.json` and
  `docs/mcr/frontend/in01-browser-journey.md`: accepted original live cohort,
  not a new prerequisite to replay that entire journey for this source delta.
- Original C06 and AR03 responsible-owner completion records remain separate
  contract/art acceptance. The controlled synthetic HTTP motion journey remains
  a consumer fixture; it does not become real backend acceptance here.

A historical execution receipt qualifies its actual source snapshot. Source
comparison and unchanged criterion dependencies can support reuse; an unrelated
new HEAD is not silently relabeled as the tested original candidate. Current
application-wide public HTTP/device/operator release acceptance is not asserted.

## Concrete bootstrap correction

The avatar mutation repository already acquires the canonical account-deletion
user lock and checks the permanent fence before personal writes. Bootstrap began
with the event lock and could insert event-local admission, participant and avatar
defaults without that check. PrivateData's later table guards do not replace the
canonical fence established when deletion starts.

The bootstrap transaction now takes the existing shared user-first advisory lock,
rechecks original authority, rejects a canonical deletion fence with Denied, then
rechecks authority before proceeding to event/admission/default rows. The lock
stays held through commit. No deletion schema, Auth saga, new provider check,
World behavior or mutable default catalog is introduced.

`native_bootstrap_honors_account_deletion_before_defaults` holds the actual
common user lock in another PostgreSQL transaction, starts bootstrap, observes
its real advisory wait via `pg_blocking_pids`, commits the permanent fence and
requires Denied. A repeated stale admission creates none of the three rows;
a different actor repeatedly creates exactly one selected default. The fence
in this component test is an explicit fixture, not a claimed Auth deletion saga.
The existing native bootstrap and restart tests remain the relevant regression
coverage for idempotence, permissions, defaults and response/clock persistence.

New helper `tests/support/bootstrap/deletion_round.py` uses the existing
marker/inode-owned migration cluster, exact hashed frozen binaries and actual
migration ledger checks. It runs only those three native tests and actual
postmaster restart, with no install, network provider or synthetic product claim.
The fresh bounded execution and exact source/result pins are recorded below.
Its resources were fully released before documentation and publication handoff.


## Reconciled original-row dispositions at source freeze

**BE04 ACCEPTED.** The actual canonical Auth owner reads active UUID/session
and optional exact provider reference in its own database. Foreign/deleted/
relinked bindings are covered by retained real owner and authenticated RPC
execution; ordinary participation omits the provider relation. Current
Participant Auth binds actor, event, session, subject and original observation
window; current SELF Profile binds the same actor and validates minimal content.
The event admission state is read under transaction locks, and suspended members
are never reactivated by bootstrap. C03 PublicProfile is built from the explicit
UUID/name/bio/authorized Concert avatar allowlist, with private canary exclusion
in the real sealed-owner/native public-reader test. Accepted IN01 supplies the
actual two-account participation boundary. These sources and evidence together
satisfy the original criteria; IN01 alone is not used as a blanket parent PASS.

The source comparison against public-reader qualified
`c04070be4219e381bd2bf2d132ac1a17a4adc73e` finds identical current application
identity, identity adapter, SELF client, Participant Auth/ports and all public
reader source except12 lines in public reader recovery dispatch. Those lines
explicitly reject four new unrelated child-reference variants; they do not
alter the positive identity/content reader. The original positive/denial tests
are reused at their original sources, not rerun or relabeled as new whole-stack
execution. Supporting native owner22, authenticated RPC5 and public-reader
three-command receipts are explicitly retained. No original BE04 code remainder
was found to justify a new identity path.

**BE07 ACCEPTED.** Original validation covers strict version/item/layer unions,
closed bounded payloads and refusal of URL/unknown inputs. Domain and application
avatar source, domain validation tests and reviewed-motion runtime test source
are unchanged from the qualified
`917b2d8042e622e4b09a8d66f9fad39a4606b4c8` producer. Its native execution covers
all15 reviewed configurations, one commit/11 conflicts under twelve concurrent
CAS requests, legacy and selected rows preserved through migration, actual
transactional witnesses and PostgreSQL restart. Existing legacy/selected native
cases cover other-user/event outfit isolation and preference revision checks.
The later real Auth/Friend/Profile/Participant join supplies actual avatar and
preference save/reload/commit/abort at `6767ee8e`; original IN01 additionally
observes the authorized peer after a real save and fresh server reread.

Current repository differences from the reviewed-motion source are the later
qualified shared-read lock correction and canonical deletion exclusion; the
avatar selection validation/update SQL and preference CAS semantics are
unchanged. Compared with accepted IN01 backend `cde817b2`, only the13-line
canonical deletion exclusion was subsequently added to this repository. It
fails closed before effects and does not alter accepted item/CAS/isolation
rules. Current release includes the actual reviewed-motion catalog, original
selected1.1 and retained layered1.0; no old artwork, arbitrary URLs, extra
presets or contract downgrade is introduced. IN01's browser theme check remains
local-only; the actual native preference persistence evidence is used for the
server criterion instead of claiming IN01 tested it.

**BE06 ACCEPTED.** Original idempotence, selected defaults, clock, complete
manifests and truthful optional capabilities have existing native and contract
consumer evidence. The missing canonical deletion exclusion is now implemented
and qualified by all three fresh native cases, current migration/ledger checks
and an actual PostgreSQL restart. The original concurrent/default/response case
passed before and after the ordering correction; the legacy restart and new
independent fence case now also pass together without weakening assertions. Current
U03 optional-field semantics, same-runtime IN04, physical-device/refidelity and
operator/release gates remain separately owned; none is silently completed by
these original backend dispositions.

These are responsible implementation-owner dispositions, not central-register
edits or king/SL acceptance. The publishing developer receives the final committed
handoff directly. The QA10 packet was separately published as
`923609a197f4ef7e4c00df11df92aa2e52cb4d99`; its actual result remains INCOMPLETE.

## Fresh BE06 validation progress

The locked/offline Rust 1.88 bootstrap-only build at `1289d90e9` passed with
exit zero in 51.754 seconds. The exact test executable is 28,528,736 bytes,
SHA-256 `c4685c8f17b5a4a11c9d8bcc74f60ea92b6ac0b20249a8278004712aa0cf787b`;
the necessary service migration executable is 64,634,912 bytes, SHA-256
`91c71b1c2b0db1e5e76a53fa2b5f5a90f68973f1e6bdf588a4b9bda2c13bb409`.
Its 2,289-input readset is SHA-256
`b80103c55e237d627feece2b057b2b8b39f6a59117e445aab8b27f31e5b81f1b`.
The previously omitted bootstrap/selected-manifest fixtures were materialized
from that exact source, and the existing domain visual-review include was
explicitly pinned. No Rust source changed to repair checkout preparation.

The first native attempt passed real PostgreSQL 16.14 migration/application
ledger checks but failed the existing fixture's root-prefix guard; the private
allocation adapter was corrected to retain the required `mcr-pg-` prefix.
The subsequent round passed `native_bootstrap_honors_account_deletion_before_defaults`
and `native_bootstrap_transactions_and_guarded_response`, both exit zero, and
physically restarted PostgreSQL. Its legacy restart assertion then observed
global participant/avatar/suspension totals `(4,4,2)` instead of `(3,3,2)` because
the independent new case had added one participant. The runner now completes
the existing concurrent-bootstrap/restart pair before the independent case.
No application behavior or assertion was weakened, and compiled Rust inputs
and both executable hashes remain unchanged. The final fresh round qualified all
three cases in this corrected order, as recorded below.

All failed reports and original allocation clocks remain preserved. Build and
native primary/independent guards passed their own resource checks; every owned
PostgreSQL root/process was removed before each literal FULL release. The late
FE37 warning was reconciled with its actual release receipt: that source job
finished 95.649 seconds before the first native acceptance, so those two jobs
did not overlap. This timestamp reconciliation does not invent a blanket
aggregate acceptance result. No serving-stack, Auth deletion saga, IN04,
device or load acceptance is claimed by these component fixtures.


## Final ordered native qualification

Executed helper source: `de93c210dc72af8662863195c85b497507599114`.
Compiled Rust source: `1289d90e9fe5bc6fee04858aee829c9e54b030ab`.
All Rust/Cargo/proto/JSON inputs are unchanged between them; only the Python
helper ordering changed within the 2,289-input execution readset, SHA-256
`e6f8113fb75370edbb06ac4fbde1d1d2795cdf35950962f43ded82a61c946a92`.
The final documentation successor does not relabel the executed source.

The earlier successful compilation command was:

```sh
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 \
  --config profile.dev.package.contracts-build.opt-level=2 \
  --message-format=json -p concert-service --test bootstrap --no-run
```

It used the explicitly allocated retained MEDIA target; no new target was created.
The ordered native round accessed neither Cargo nor that target. Its guarded
controller invoked the committed helper through the private custody adapter with
`--test-binary`, `--test-sha256`, `--service-binary`, `--service-sha256`,
`--pg-bin` and `--report` bound to the frozen binaries above, local PG16.14 tools
and the new owned proof root. Each exact native test used
`bootstrap --ignored --exact TEST_NAME --test-threads=1`.

| Actual command/operation, in order | Result |
| --- | --- |
| Frozen service `--migrate` and source SHA-384 SQL-ledger comparison | exit 0, PASS |
| Frozen service `--verify-migrations` and ledger comparison | exit 0, PASS |
| `native_bootstrap_transactions_and_guarded_response` (16 concurrent callers) | exit 0; 1 passed, 0 ignored |
| Physical stop/start, postmaster PID `9247` → `9859`; ledger comparison | PASS |
| `native_bootstrap_restart_persistence` | exit 0; 1 passed, 0 ignored |
| `native_bootstrap_honors_account_deletion_before_defaults` | exit 0; 1 passed, 0 ignored |
| Owned PostgreSQL cleanup and guarded controller | PASS; exit 0 |

The actual allocation was `BE06-ORDERED-COPIED-NATIVE-20261104T2320Z`.
ACCEPT: `2026-11-04T23:32:17.189554Z` (`1793835137.189554`). Original work
and FULL deadlines were `23:35:17.189554Z` and `23:37:17.189554Z`.
Actual FULL: `1793835156.621578`, before both deadlines. Workflow, remaining
NEW reservation and owned PIDs were all zero; the owned PG root was absent.
The literal release went directly to allocator21d and successor FE37@230 before
this documentation, with no manager inspection, approval or acknowledgment gate.

Entry pinned actual Root153 SOURCE2315, corrected QA04 SOURCE2325, Gift
CONTROLS2327, FE41 browser2320 and Map native2320 releases. A failed predecessor
was used only as a resource release, never as product acceptance. The accepted
snapshot reserved OTHER5GiB plus own NEW512MiB with fresh free18,513,772,544 bytes;
it enforced the 11GiB start, 9GiB after-all-growth and 8GiB live floors. Own limits
were 40ALL/3GiB RSS, NEW512MiB, WHOLE6GiB, work180/FULL300/cleanup120 seconds.
The inode-deduplicated actual prior carry was2,038,984,704 bytes; its charge stayed
at the nonshrinking3GiB floor. Primary and separate independent LIVE guards
started before the native child, included owned daemonized PostgreSQL by exact
marker/inode/postmaster identity and descendants, and both ended with null failure.
The accepted sibling upper was48ALL/10.5GiB, aggregate88ALL/13.5GiB/G3. This is
the admitted snapshot, not a claim of independently inspecting peers' workloads.

Across the two guards, own peaks were13 processes,267,190,272 RSS bytes,
54,427,648 NEW bytes and3,275,653,120 WHOLE bytes; minimum free12,140,900,352 bytes.
All retained failed attempts and their original clocks remain immutable.

Raw local receipts remain under
`/private/tmp/mcr-be06-native-ordered-20261104/`:

| Receipt | SHA-256 |
| --- | --- |
| `native-report.json` (all commands, outputs, migration ledger, restart and cleanup) | `f3fa63a2d6be5cbf642f5f2c4d44a14ce12fd246f0f9bb429f6f3bdb625bb7f7` |
| `FULL.json` | `f725c14d5016242c41818934fb9a067f36e4c1ed1de0749f87fa29184b19c5ab` |
| `guardian.result.json` | `a4ef3f79929bdc5c349f1637cb5a19c5d4fe471c553c87b72f08fc2b9dfa6f96` |
| `acceptance.json` | `5dfb7ddc0d2c9bcb0250034c373d84aad512745d01584303a6ab8a914c824dfe` |
| `request.json` (frozen controls and predecessor receipts) | `69bc773768ee118b816de16efa4a16e3d3006bd4765d21243004ab59db37596b` |
| `capacity.json` | `5f95501e92d84ad48334da973064ffae179e89d2c62936a7b645d6be449c5e9c` |

The successful build's FULL receipt remains at
`/private/tmp/mcr-be06-bootstrap-1289-retry-20261104/FULL.json`, SHA-256
`642933424a870f737af7cc6a599e2de130903371a068088af2817be64ede784a`.
No new network, dependency, migration, artwork, rendering or shared contract is
introduced. Auth deletion-saga/public HTTP, IN04, actual managed/operator,
physical-device/refidelity and user-deferred QA08 load acceptance remain separate.
The final patch contains only bootstrap repository exclusion, its direct native
test, the dedicated helper and this completion record. BE04/07's unchanged
original criteria remain ACCEPTED; BE06's corrected original criteria are now
ACCEPTED by this responsible implementation owner.
