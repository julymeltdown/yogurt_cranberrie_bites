# Original FE34 remainder

Status: IN_PROGRESS. This assignment does not reopen the completed Profile media
delivery and does not claim full FE34, U03, live persistence or visual acceptance.

Implementation owner: `01a1a213-61d6-7a20-a8e7-476649312f8c`.
Exclusive checkout: `fe-reference-frame-footer`.
Branch: `codex/mcr-fe-FE34-profile-preferences-completion`.
Base: `a10e4f409f5727cb25b6cc37d75f499a73e9bb70`.
Persistent unbudgeted goal: created `1793819098`; active through implementation,
relevant validation, fixes, documentation, committed handoff and normal Root153
publication. Developers own validation; no king/SL review or acknowledgment gate.

## Requirements and actual gaps

The implementation plan defines U01 as ME with avatar/profile/activity, U03 as
nickname, bio, optional ticket section and visibility with validation, and U07 as
theme, reduced motion, low power and sound off by default. FE34 additionally
requires optional profile fields not to block World entry and identity-scoped
private settings. The implementation plan remains authoritative.

Current model/controller, ProfileForm, SettingsPage and activity links were read
at the base above. Existing behavior already includes scoped participant parsing,
section-specific saves/cancel, retained drafts after conflict or uncertain save,
revision checks, privacy cleanup and saved-preference-only explicit application.
These are retained implementations, not new gaps.

Two gaps were established:

1. Draft validation accepts whitespace-only names and control characters which
   the existing canonical Profile owner rejects. The assigned frontend model and
   ProfileForm hunks can fix this independently of a new contract.
2. Own ticket section and visibility are absent from both the strict C03
   participant/update shape and the delivered producer. The current form
   truthfully reports their unavailability. An optional render extension is not
   persistence. The specification does not define what visibility covers, its
   audiences or its default; those semantics must be resolved before an expanded
   contract is frozen. Party ticket sections and inherited account visibility do
   not establish these own-profile semantics.

The actual Participant update/public composition owner is Root1fc,
`01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08`; this was confirmed directly. ProfileSELF
owner `01a1a1fd-6b5d-75b0-8d42-01ade79b9393` owns display name/bio persistence.
Legacy232 and frontend209 are not the Participant producer. No frontend-only
field, audience flag, unsupported PATCH key or successful persistence is inferred.
Root1fc has asked the user whether visibility covers ticket section alone or bio
and ticket section, with private default/event-member opt-in as the proposed
audience model. The answer remains pending; this is a product-semantics question,
not a management approval or runtime dependency. Root1fc will freeze the public
GET/PATCH extension and persisted revision/clear/error behavior after that answer.

## Existing contract alignment

Read-only producer source:
`backend/services/profile-service/src/concert_self_profile/model.rs` at
`6767ee8e805db17291d1d77a25c72d881e9f743a` in the retained
`be-participant-producer` checkout. `valid_name` requires a nonblank Unicode-trimmed
name, at most 40 Unicode scalars and no `char::is_control`. `valid_bio` permits at
most 160 scalars and no controls. Nullable bio and explicit clearing remain valid.
These are source facts, not a fresh public Gateway runtime result.

The new frontend draft validation mirrors Unicode White_Space and Cc categories,
retains the existing scalar/surrogate checks and does not normalize submitted
values. Invalid drafts stay local and editable, with actionable live feedback;
they do not start a PATCH or become an uncertain-save block. Corrected values
use the existing revisioned PATCH. Local cancel still makes no server rollback.
Participant decoding and the public wire shape are unchanged.

Assigned production changes are only profile/preferences model hunks in
`frontend/src/features/me/model/{contract,controller}.ts` and ProfileForm content
in `frontend/src/features/me/ui/MePages.tsx`. MeHub, activity/actions, SettingsPage,
CSS, media, art, avatar, World, Auth, root composition and shared contracts remain
outside this write lease. New tests/guardian use
`frontend/src/features/me/__tests__/original-fe34/`; this document and
`docs/mcr/evidence/original-fe34/` are the single new documentation/evidence prefix.
No source archive, dependency, manifest or lockfile changes are introduced.

The bounded source guardian is adapted from this checkout's retained
`profile-activity-media/source-round.py` at the base above, with a new private
cache/evidence prefix and only the new validation tests, affected Me regression
tests, no-emit types and scoped lint. It runs no historical media/native matrix.

## Retained qualification and limits

Reuse the qualified profile/controller behavior in `profile-preferences.md` and
`account-entry.md`; runtime producer/adapter behavior in
`world-runtime-preferences.md`, `fe34-runtime-preferences.md` and
`runtime-preferences-root-integration.md`; media in `profile-activity-media.md`.
Responsible runtime producer26e supplied publication `1bb911c3c8594a997e7a309d86b823d906198b95`
with 40 owned Git blobs/39 payload SHA256 pins, producer mapping
`c16c0c875b446f494137b72bc1feee6f27fdbe14` to
`f08821890e6e11e61b35bb333897b476faa12083`, and native03 report hash
`ec6ff9191ae5312930034d70f66fdcbcb4d36365cdb91d8653b243aec5409cc1`.
Those retained controlled-HTTP/Phaser results cover save-only versus explicit
apply, reduced motion, low power, theme failure/success and privacy/route cleanup.
They do not qualify current live GET/PATCH persistence, physical-device battery
benefit or an audible source. No new runtime lease or replay is needed for reuse.

## First focused source round and required fixture repair

`FE34-EXISTING-CONTRACT-SOURCE-20261104T1932Z` was accepted at
2026-11-05 04:33:58.236792 KST with 120 seconds work and 60 seconds cleanup.
The guardian's mechanical carry fix included the borrowed physical dependency
tree in addition to four retained checkouts and the private Git reserve. Fresh
free 14,259,068,928 bytes minus other outstanding growth 2,348,810,240 and own
128 MiB left 11,776,040,960 bytes, above the 9 GiB residual requirement. The
12 GiB start/8 GiB live floors and the 2 GiB whole-footprint cap remained intact.

The selected files expanded to 28 actual cases: 15 existing controller cases,
three existing UI cases and ten new validation cases. Six passed and 22 failed;
the common failure occurred during initial loading, before the new validation.
Types and lint did not run after the failed tests. The retained setup supplied
the Me response to every request, but canonical Auth now first requests the
strict enrollment admission DTO. It therefore rejected that response before
requesting the participant. This is a stale controlled fixture, not proof of a
public runtime defect or a passing new validation result.

Actual FULL was 04:34:15.482360 KST: descendants, dependency link and private TMP
were gone before the original deadline. Peaks were four processes, 733,008 KiB
RSS, 2,027,442,176 bytes whole footprint and 14,127,219 bytes TMP. Raw failure,
source hashes, admission and cleanup remain under `source-01/` and
`source-01-admission.json`; the successor received the returned slot directly.

Root153 subsequently transferred only the existing `__tests__/fixtures.ts`
`setup(fetcher)` transport-construction hunk and its necessary import. The new
controlled helper supplies a strict ADMITTED DTO only for the registered
admission GET with the fixture credential, and delegates application requests to
the original feature fetcher. Canonical Auth still requests and decodes
admission. Production admission code and all existing assertions are unchanged.
The repair requires a fresh finite source round; no old allocation is reused.
No source/browser/backend resources are held. The new validation change is not
yet a delivered or accepted whole-feature result.

The subsequent `FE34-EXISTING-CONTRACT-RETRY-20261104T1955Z` offer was declined
before acceptance or children at 2026-11-05 04:59:48.355832 KST. Free space was
12,724,441,088 bytes, below its explicit 12 GiB start floor of 12,884,901,888.
The conservative projected residual was 11,038,330,880 bytes, above 9 GiB; that
does not waive the separate start floor. Its avatar predecessor also declined
before acceptance, so no actual FULL was inferred. No guardian, dependency link,
temporary directory, test or compiler started. The unchanged repaired candidate
remains queued for a fresh usable offer. Allocator and named artist successor
were notified directly; `source-02-declined.json` preserves the exact preflight.

## Qualified validation delta

The allocator closed the declined 1955 offer and issued fresh
`FE34-EXISTING-CONTRACT-RETRY-20261104T2005Z` after the new avatar job's literal
FULL at 2026-11-05 05:01:59.665985 KST. This new offer explicitly authorized an
11 GiB start floor, retaining the 9 GiB aggregate residual and 8 GiB live floors,
all process/footprint/growth caps and 120/180-second absolute clocks. The guardian
now accepts an explicit floor parameter and defaults to the original 12 GiB;
historical guards and the earlier decline remain unchanged.

Actual acceptance was 05:02:31.388131 KST. Fresh free 12,824,477,696 bytes minus
the allocator's conservative other growth of 2,818,572,288 and own 128 MiB left
9,871,687,680 bytes, above 9 GiB. The full physical carry, including retained
dependencies, was 2,013,048,832 bytes; the conservative prior maximum was retained
separately as 2,027,442,176. No installation or dependency change was made.

All commands exited zero on source `1d70772bcbafdfcac5fb40a3bb526095c0cd68a4`:

- The same three selected files passed all 28 tests: ten new validation cases,
  15 existing controller cases and three existing UI cases. This qualifies blank
  and control rejection, retained editable drafts, corrective exact-value save,
  local cancellation and the affected existing scope/conflict/revision behavior.
- Ordinary full application TypeScript passed with no emit and incremental
  output disabled, retaining the already documented import-extension flag.
- Scoped lint with warnings denied passed for the three changed production
  files, the transferred fixture hunk and the new test/helper prefix.

Exact commands, source SHA256 pins and raw logs are in
`source-03/guardian.json` and `source-03/*.log`; admission and actual predecessor
are in `source-03-admission.json`. Actual FULL was 05:02:54.528556 KST, before
the original 05:05:31.388131 deadline. Peaks were four processes, 1,684,496 KiB
RSS, 2,027,491,328 bytes whole footprint and 14,135,075 bytes TMP. All recorded
descendants, temporary dependencies and private TMP were removed. The artist
successor and allocator received the actual FULL directly; no acknowledgment or
new qualification stage is required.

This is a qualified existing-contract frontend validation change and controlled
fixture repair. It does not certify live admission, public persistence, native
visual acceptance, ticket section, visibility or whole FE34. Publication is
routed directly to Root153. The original field extension still needs the user's
visibility target/audience decision and Root1fc's frozen producer contract; no
unsupported fields have been sent and the full goal is not complete.
