# Original BE04, BE06 and BE07 completion

Status: **IN_PROGRESS**. This is the responsible developer's current reconciliation
and implementation record, not a replacement specification or release decision.

## Durable objective and custody

Finish original BE04 canonical identity/private projection, BE06 idempotent
bootstrap/defaults/capabilities/manifests/clock, and BE07 validated avatar/theme
persistence with optimistic concurrency. Reuse existing source and qualified
native/product evidence; implement genuine remaining behavior, perform relevant
developer-owned validation and fixes, document provenance and limits, and deliver
a committed handoff directly through named publisher Root1fc. Each original row
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
| BE06 | Idempotent participant/default preset, capabilities, manifests, server clock; one participant under repetition, truthful optional degradation | Bootstrap repository and release binder; native16 concurrent calls, suspension/rollback/deadline/restart; selected six-preset binding; bootstrap feature consumer tests and accepted IN01 | IN_PROGRESS: canonical deletion exclusion was missing before bootstrap defaults. New fix and native regression pending allocated execution. |
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
Execution, results, source pins and final row dispositions are pending the
allocator's fresh bounded backend/native offers. No scarce resource is held
by source preparation.


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

**BE06 IN_PROGRESS.** Original idempotence, selected defaults, clock, complete
manifests and truthful optional capabilities have existing native and contract
consumer evidence. Only the newly fixed canonical deletion-exclusion behavior
and its relevant native regressions remain pending execution for this delivery.
The row will be disposed after that actual run and any necessary fixes. Current
U03 optional-field semantics, same-runtime IN04, physical-device/refidelity and
operator/release gates remain separately owned; none is silently completed by
these original backend dispositions.

These are responsible implementation-owner dispositions, not central-register
edits or king/SL acceptance. The publishing developer receives the final committed
handoff directly. The QA10 packet was separately published as
`923609a197f4ef7e4c00df11df92aa2e52cb4d99`; its actual result remains INCOMPLETE.
