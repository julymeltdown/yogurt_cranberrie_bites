# Source register and provenance

Status: **PLANNING_COMPLETE / IMPLEMENTATION_NOT_STARTED**.

Machine-readable companion: [source-register.json](source-register.json).

## 1. Pinned revisions

| Repository | Revision | Role | Where |
| --- | --- | --- | --- |
| `julymeltdown/mcr` | `a2b10bf92d716bdb5d0ca85b481de6265b466bf7` | Integration foundation; the entire seed tree | `origin/master` of the MCR checkout |
| `julymeltdown/yogurt_cranberrie_bites` | `03b673224aaa77842d763f817a660eb2d6f634d6` | Delivery base: one empty commit on top of the MCR revision | `yogurt/master` |
| `julymeltdown/bigyeting` | `dd47ed502edd5d4f9bc4b726e9088b1ee0f0547b` | Documented baseline cited by the product plan | Upstream |
| `julymeltdown/bigyeting` | `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a` | Audited checkout used for every file and symbol verified in this set | `/Users/lhs/Desktop/mcr-sources/bigyeting` |
| `julymeltdown/port-rust` | `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2` | Rust service foundation, already imported into MCR | `/Users/lhs/Desktop/mcr-sources/port-rust` |

## 2. What was actually verified while writing this set

| Claim | How it was checked |
| --- | --- |
| The yogurt repository was empty and has no `master` branch | `git ls-remote` returned nothing before the push; the GitHub API reported `size: 0`, `default_branch: main` |
| The seed commit is empty | `git commit --allow-empty` on a clone of MCR `a2b10bf9`; `git log` shows the parent and no file change |
| MCR `master` equals the documented baseline | `git rev-parse HEAD` → `a2b10bf92d716bdb5d0ca85b481de6265b466bf7`; `git ls-remote origin master` matches |
| MCR's planning check still passes on the seed tree | `python3 scripts/mcr/check-plan.py` exited 0 |
| Every Bigyeting file cited in [migration-plan.md](migration-plan.md) exists | File-by-file `wc -l` and symbol grep against the audited checkout |
| Every MCR file cited in [migration-plan.md](migration-plan.md) exists | File-by-file `wc -l` and symbol grep against the seed tree |
| MCR has no gender field anywhere | `grep -ril gender` over `backend/`, `contracts/`, `frontend/src` returned only two documentation files that describe its removal |
| MCR's intro entity has paired age bounds and no gender | `contracts/domain/v1/social.schema.json` `AgeBounds`; `concert-service/migrations/0049_intro_native_witness.sql` |
| MCR's avatar is keyed by `(event_id, user_id)` | `backend/crates/concert-domain/src/lib.rs` `ParticipantKey` |
| Bigyeting's like flow runs through the music-taste-event module | `adapters/in/http/music_taste_event.rs` `like_intro`/`unlike_intro`/`set_intro_like` |
| Bigyeting's reward trigger exists | `migrations/0046_music_taste_event.sql` |
| The C08 guard rejects Bigyeting-only rendering dependencies | `scripts/mcr/check-ui-origin.mjs` `forbiddenImport` |

Claims about behavior that were **not** executed are not listed. In particular,
no integrated build, no database migration, and no end-to-end flow has been run
for the new features.

## 3. Baseline reconciliation rule

The product plan links to `dd47ed5…`; the local audited checkout is `5ee…`. They
are different commits. Before adapting any file:

1. Fetch the documented baseline (`git fetch` the pinned commit, or read it
   through the GitHub API at that SHA).
2. Compare the file at `dd47ed5…` with the file at `5ee…`.
3. If identical, adapt from either and record `5ee…` as the inspected revision.
4. If different, read both, adapt from the **documented** baseline `dd47ed5…`,
   and record the difference and the reason in the commit message.
5. If the file does not exist at `dd47ed5…`, it is not a valid source. Do not
   adapt it; re-author the behavior.

Record the outcome in §5 below for every adapted file.

## 4. Provenance obligations per adapted file

Every commit that adapts donor code must state:

- source repository and revision,
- source path,
- destination path,
- the symbols taken,
- what was removed (event coupling, provider coupling, reward coupling,
  identity fields),
- the destination test that covers the adapted behavior.

Example commit body:

```text
feat(intro-likes): port heart toggle from Bigyeting

Source: julymeltdown/bigyeting @ dd47ed502edd5d4f9bc4b726e9088b1ee0f0547b
Path:   backend/services/bigyeting-api/src/application/use_cases/intro_likes.rs
Symbols: change_intro_like, SetIntroLikeCommand, SetIntroLikeOutcome
Removed: active_x_account_id actor resolution, event coupling
Tests:  backend/services/concert-service/tests/intro_likes_contract.rs
```

## 5. Adaptation ledger

Fill one row per adapted file as work lands. Empty until implementation starts.

| Destination | Source repo | Source revision | Source path | Symbols | Removed | Tests | Commit |
| --- | --- | --- | --- | --- | --- | --- | --- |
| — | — | — | — | — | — | — | — |

## 6. Licensing and redistribution

Neither MCR nor Bigyeting carries a tracked LICENSE/COPYING/NOTICE file, and the
Bigyeting package manifests omit a license field. The user authorized this
internal reuse. That authorization is not a redistribution license. Before any
public release of the integrated application, resolve the license position for
every adapted file and for the third-party dependency set, and record the
conclusion here. This register does not make a licensing determination.

## 7. Preserved artifacts

These must remain byte-identical:

| Artifact | Reason |
| --- | --- |
| `docs/source/` (MCR archive) | Pinned by `scripts/mcr/check-plan.py` checksums |
| `MCR_IMPLEMENTATION_PLAN_V2_EN.md`, `PLAN_TABLES_EN.md`, `MCR_EXPORT_PLAN_EN.py`, `TEST_PLAN_EXPORT_EN.py` | Inputs to the planning check |
| `docs/mcr/source-provenance.json` | Pinned narrative checksum |
| `frontend/src/entities/avatar/character-source-pack.json` | Character art authority |
| `frontend/public/world/avatars/derived-v1/motion-candidates-v1/*` | Referenced by `art.ts` path and hash contracts |
| `docs/mcr/design/reference-target-2026-11-04.png` | Visual target |
