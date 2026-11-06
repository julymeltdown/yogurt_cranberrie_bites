# F1 - Plan compliance audit

Executed: 2026-11-06
Plan: `.omo/plans/mcr-finish-project.md` (684 lines, rows 1-51 plus the F1-F5 final verification wave)
Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-f1`, branch `codex/mcr-exec-f1`
Base: `33141de3ff25bccab0275b796197fb13e9fa879f` (`chore(plan): close the CI restoration row`)
Reused input: `.omo/evidence/mcr-finish-project/task-47-in05.json` (candidate `7d14251bb`, generated 2026-11-06T18:17:43Z) for evidence-existence and per-row delivering-commit pointers only. Every delivering-commit claim was then re-checked independently against the base with `git merge-base --is-ancestor <sha> 33141de3f`.

## Method

1. Parsed every plan row header (`- [x]`, `- [~]`, `- [ ]`), its `Evidence ...` path and its `Commit:` line directly from the plan file at the base commit.
2. Resolved each evidence path in the worktree; recorded a substitution whenever the plan path did not exist as written.
3. Took the IN05 per-row delivering commit, plus the plan-closure commits found with
   `git log -S"- [x] <row>. <title>" -- .omo/plans/mcr-finish-project.md`, and ran
   `git rev-parse <sha>^{commit}` and `git merge-base --is-ancestor <sha> 33141de3f` for each.
4. Scanned every evidence artifact for cited commit SHAs and checked those that resolve in this repository.
5. Quoted every `[~]` row's gap text verbatim and judged whether it names an external/product limit or an internal gap that remains open work.

## Result summary

| Measure | Count |
| --- | --- |
| Rows 1-48 | 34 `[x]`, 14 `[~]`, 0 other states |
| Rows 49-51 (appended batch) | 49 `[x]`, 50 `[x]`, 51 `[~]` |
| Rows 1-51 combined | 37 `[x]`, 14 `[~]`, 0 other states |
| Evidence artifacts present at the plan path | 51/51 |
| Evidence substitutions required | 0 |
| Delivering-commit ancestry checks run | 54 checks: 47 distinct ledger commits covering 48 rows, plus 3 for row 11 and 2 each for rows 47 and 48 |
| Ancestry failures | 0 |
| Gap rows whose gap text names the missing piece | 14/14 |
| Final verification wave | F1 = this artifact (in progress); F2-F5 open, evidence not yet written |

Overall F1 verdict: **COMPLIANT-WITH-RECORDED-GAPS**. No implementation row is in an unaccounted state, no evidence artifact is missing, no delivering commit is outside the base, and no `[~]` row hides an unnamed narrowing. This is not a release verdict: 14 named gaps remain open and F2-F5 have not run.

## Per-row table

Evidence paths are relative to `.omo/evidence/mcr-finish-project/`; commits are abbreviated to 9 characters (full SHAs are in `task-47-in05.json` and in the evidence files). "ok" in the ancestry column means `git merge-base --is-ancestor <full-sha> 33141de3f` exited 0.

| Row | State | Evidence path | Commit | Ancestor | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | [x] | `task-1-frontend-commit.md` | 8e5cc757f | ok | PASS |
| 2 | [x] | `task-2-backend-commit.md` | 8fc507fe9 | ok | PASS |
| 3 | [x] | `task-3-ci-removal.md` | 501710d29 | ok | PASS |
| 4 | [x] | `task-4-candidate.md` | - (plan `Commit: N`) | ok | PASS |
| 5 | [x] | `task-5-salvage-table.md` | 49ac04156 | ok | PASS |
| 6 | [x] | `task-6-ci-inventory.md` | 0dfbf3d8b | ok | PASS |
| 7 | [x] | `task-7-be37.md` | c6051f812 | ok | PASS |
| 8 | [x] | `task-8-be15.md` | 004949bcf | ok | PASS |
| 9 | [x] | `task-9-be03.md` | 81dfe8df1 | ok | PASS |
| 10 | [x] | `task-10-admission.md` | 1980fb0c7 | ok | PASS |
| 11 | [~] | `task-11-media-feed.md` | d962c7d2f (+39512c69a, 53d547059) | ok | GAP (named) |
| 12 | [x] | `task-12-parity.md` | 4465e3f6f | ok | PASS |
| 13 | [x] | `task-13-stack.md` | d6de9bae6 | ok | PASS |
| 14 | [~] | `task-14-be42.md` | 3f3c6331f | ok | GAP (named) |
| 15 | [x] | `task-15-be38.md` | ccfc9fc0e | ok | PASS |
| 16 | [~] | `task-16-be41.md` | 1bc74e3b8 | ok | GAP (named) |
| 17 | [x] | `task-17-be43.md` | 800c830a6 | ok | PASS |
| 18 | [x] | `task-18-venue-policy.md` | bc452a9cf | ok | PASS |
| 19 | [x] | `task-19-venu-l01.md` | 47478ed51 | ok | PASS |
| 20 | [x] | `task-20-venu-m01.md` | 181e13da5 | ok | PASS |
| 21 | [x] | `task-21-venu-n01.md` | 3489b6934 | ok | PASS |
| 22 | [x] | `task-22-venue-tests.md` | 3fa4f939a | ok | PASS |
| 23 | [x] | `task-23-venue-authz.md` | 42b48301e | ok | PASS |
| 24 | [x] | `task-24-shell-ports.md` | 627f62b75 | ok | PASS |
| 25 | [x] | `task-25-map-friends.md` | e61dd7f98 | ok | PASS |
| 26 | [x] | `task-26-invitations.md` | 1e590013e | ok | PASS |
| 27 | [x] | `task-27-notes.md` | 330853af5 | ok | PASS |
| 28 | [~] | `task-28-venu-in01/` | a9dc07eab | ok | GAP (named) |
| 29 | [x] | `task-29-in02.md` | f9f833d6e | ok | PASS |
| 30 | [~] | `task-30-in03.md` | 0e5ba4df1 | ok | GAP (named) |
| 31 | [x] | `task-31-in04.md` | be15c23ef | ok | PASS |
| 32 | [~] | `task-32-in01-delta.md` | 52b1e4542 | ok | GAP (named) |
| 33 | [x] | `task-33-pi08/` | dad8c9126 | ok | PASS |
| 34 | [x] | `task-34-pi11/` | 600a8f809 | ok | PASS |
| 35 | [~] | `task-35-ar01.md` | 39a1b4e04 | ok | GAP (named) |
| 36 | [~] | `task-36-qa06/` | ebab71974 | ok | GAP (named) |
| 37 | [~] | `task-37-atlas.md` | cc9691cec | ok | GAP (named) |
| 38 | [x] | `task-38-qa03.md` | 5c592ea56 | ok | PASS |
| 39 | [x] | `task-39-qa04.md` | ff835cd32 | ok | PASS |
| 40 | [x] | `task-40-qa05.md` | d37025cb3 | ok | PASS |
| 41 | [~] | `task-41-qa09.md` | 593ea8976 | ok | GAP (named) |
| 42 | [~] | `task-42-qa10.md` | a9bbedbec | ok | GAP (named) |
| 43 | [~] | `task-43-fe-closure.md` | 8983271a9 | ok | GAP (named) |
| 44 | [x] | `task-44-deployment.md` | 847c75b6b | ok | PASS |
| 45 | [~] | `task-45-fe10.md` | 39a1b4e04 | ok | GAP (named) |
| 46 | [x] | `task-46-w04.md` | 9aefc3b14 | ok | PASS |
| 47 | [x] | `task-47-in05.md` | 7bbcba836 / ef02302d4 (content / plan closure) | ok | PASS |
| 48 | [x] | `task-48-ci.md` | bbf9da478 / 33141de3f (content / plan closure) | ok | PASS |
| 49 | [x] | `task-49-feed-mutations.md` | a5389a857 | ok | PASS |
| 50 | [x] | `task-50-deletion-mount.md` | 79d18315e | ok | PASS |
| 51 | [~] | `task-51-closure-producer.md` | 2e4d7fc90 | ok | GAP (named) |

Row 4 is a verification-only row (`Commit: N` in the plan) and claims no commit; its evidence records the clean-candidate command matrix. Rows 47 and 48 were still `[ ]` in the IN05 snapshot and are `[x]` at the base: 47 published by `7bbcba836` and closed by `ef02302d4`, 48 delivered by `bbf9da478` and closed by the base commit `33141de3f` itself.

## Appended rows and final verification wave

| Row | State | Accounting |
| --- | --- | --- |
| 49 | `[x]` | Evidence `task-49-feed-mutations.md` exists; delivering `a5389a857` is an ancestor. Closes the feed-mutation half that row 11 named as its gap. |
| 50 | `[x]` | Evidence `task-50-deletion-mount.md` exists; delivering `79d18315e` is an ancestor. Closes the serving-mount half that row 14 named as its gap. |
| 51 | `[~]` | Evidence `task-51-closure-producer.md` exists; delivering `2e4d7fc90` is an ancestor. Carries the residual closure/archive gap that row 16 deferred. |
| F1 | `[ ]` | This audit. Evidence `f1-compliance.md` is the file this run writes. |
| F2 | `[ ]` | Not executed. `f2-quality.md` absent, as expected for an unrun wave item. |
| F3 | `[ ]` | Not executed. `f3-manual-qa/` absent. |
| F4 | `[ ]` | Not executed. `f4-ideal-state.md` absent. |
| F5 | `[ ]` | Not executed. `f5-release.md` absent; F5 owns the refreshed 111/112/61/12/8/10 reconciliation. |

The wave is therefore accounted for: each of F1-F5 has an explicit open state, none is marked done without its artifact, and F1 is the item this task executes. The plan's own gate ("ALL must APPROVE ... wait for the user's explicit okay before declaring complete") is untouched.

## Gap analysis (every `[~]` row, verbatim quote and judgement)

No user-authorized scope cut exists in rows 1-51. The only `DEFERRED_BY_USER` item in the delivery is QA08 (recorded by row 47, outside the numbered rows). Each gap below is a named, open piece of work.

### 11 - "media delivered; feed mutation transport gated on the shared proto envelope"

> Complete BE24/BE31/BE32 media, feed and post composition (media delivered; feed mutation transport gated on the shared proto envelope) ... Scope result: media composition verified and merged; feed mutation transport remains deliberately closed. Gap tracked by task 49.

Judgement: internal, and since closed by row 49 (feed mutation envelope and routes, delivering commit `a5389a857`). Not a reduction: the row names the missing half and the follow-up row. The row could defensibly be flipped to `[x]` now that 49 has landed; leaving it `[~]` is conservative under-claiming.

### 14 - "coordinator + sealed apply delivered; serving binary does not yet mount it"

> Complete BE42 sealed account-deletion apply (coordinator + sealed apply delivered; serving binary does not yet mount it)

Judgement: internal, and since closed by row 50 (serving mount, delivering commit `79d18315e`). Not a reduction; same under-claiming note as row 11.

### 16 - "route + read-only proofs delivered; closure producer and archive HTTP endpoint unmounted"

> Complete BE41 event closure and archive (route + read-only proofs delivered; closure producer and archive HTTP endpoint unmounted)

Judgement: internal, and the named piece was delivered by row 51; the residual gap now lives on row 51. Not a reduction.

### 28 - VENU-IN01

> 48/48 reference cases and 10/10 IA states have captures and verdicts (23 PASS / 2 DIFFER / 23 NOT_REACHABLE; 3 PASS / 7 NOT_REACHABLE), but 23 cases and 7 states are unreachable while PgVenueLocationAuthority::friend_in is a fail-closed stub returning VenueLocationError::PolicyUnavailable (503). The two DIFFER cases are genuine reference-base differences (provisional Photo 5 vs Photos 1 and 4) and remain unapproved. A bilateral friend authority is being implemented; the full matrix must be re-run against it to convert those families (friend location, sharing audience, meeting, invitation, conversation).

Judgement: genuine product gap inside this delivery, not an external limit and not a user-approved narrowing. The 48-case matrix is complete and honest (23 PASS / 2 DIFFER / 23 NOT_REACHABLE), but the friend-authority stub means almost half the venue comparisons cannot be reached. Remains open work.

### 30 - IN03 social and privacy gate

> 27/27 executed checks pass on a real stack with exact envelopes (follow/request/decline/accept, private-profile fence and invalidation, block/unblock with journal rows, blocked follow 403, feed excluding deleted and blocked authors, wall outsider 404, 11-denial payload audit with zero private-metadata leaks, and the blocked-read feed now 200 with items=[] instead of 503). NOT_RUN with named blockers: four provider cases (age provider/claims, OAuth provider, external mail/SMS), age-provider positive and social reveal/revoke, venue meeting/location cases (the branch predates the venue dispatch merge and must be re-run), no blocked-comment seed for blocked_comment_author_filter, and no browser/WS client for client_cache_and_ws_frames.

Judgement: mixed. The four provider cases, age-provider positive and social reveal/revoke are genuine external inputs (provider provenance, external mail/SMS). The venue meeting/location cases and the missing blocked-comment seed are internal re-run work. 27/27 executed checks pass; the gap is disclosed with each NOT_RUN item named.

### 32 - IN01 deltas

> D1 admission, D3 panel, D4 theme, D5 panels and the socket-kill reconnect PASS on real browser evidence with one socket per client and zero invalid frames; D2 (policy frames) is NOT_RUN because the Social policy baseline transport is not composed in this gate (CONCERT_SOCIAL_MODE unset with no probe decoder), and feed CONTENT is NOT_RUN because the Feed product mode is off. Sustain refreshObservation was not observable inside the runtime window (900 s token TTL).

Judgement: internal composition gap (Social policy baseline transport and Feed product mode are not composed in that gate), plus one observability limit. Not external, not authorized as a cut.

### 35 - AR01 venue provenance

> no reviewed AR01 inputs supplied. Binding map and exact missing inputs recorded in .omo/evidence/mcr-finish-project/task-35-ar01.md; unverified/closed places stay refused at domain, DB, repository and application layers (fail-closed proven). Three schema gaps need a contract decision, not data: no place publication-expiry field, no closure-owner field, no event-day layout date.

Judgement: genuine external input (no reviewed AR01 material supplied) plus three schema fields that need a contract decision. Correctly fail-closed; the unverified places are refused rather than fabricated.

### 36 - QA06 mobile visual acceptance

> the matrix is honest but incomplete - 206 desktop case-widths at 360/390/430 (95 PASS, 0 DIFFER, 111 NOT_REACHABLE, 0 FAIL) plus 36 Android-emulator captures from an earlier session (not re-run at the delivered commit; physical iOS/Android NOT_RUN with no simulator or device). Blocked families with named producers: reviewed siteplan/landmark (W02/W03/W07/M06), community clock/entry authority (G01-G07, M01-M05), world mural-scene release (R01, PI09), operator/moderator authority (O01-O09), safety service (U08/S01), requests/social provider (U04/U05), email/OAuth/legal/archive (A03/A04/A06/A07/X01). Family capture coverage is 8 distinct cases and 15 aliases (identical bytes to their IA base); only W01-parade, PI08-danger-era and the two PI01 chrome cases add pixels. Two 44px defects were fixed (profile back-world link 21px, admin check-access 25px).

Judgement: mixed. Physical iOS/Android is a genuine external limit (no device or simulator), and the blocked families are named internal producers (siteplan/landmark, community clock, mural scene release, operator authority, safety, social provider, email/OAuth/legal/archive). 95 PASS / 0 DIFFER / 0 FAIL with 111 NOT_REACHABLE is an honest incomplete matrix, not an approved narrowing.

### 37 - character atlas production status

> reviewed motion manifest mcr_selected_motion_reviewed_20261104 and its 1280x1856/576-cell atlas are produced, hash-pinned, guarded and loader-wired, but are NOT served: compose sets no CONCERT_GRPC_ADDR and none of the six CONCERT_* release settings, the reviewed bootstrap exists only as a runtime test fixture, and reviewed-handoff.json records production_activation NOT_RUN. Production activation and a served-renderer capture remain open with the World release path (row 51). Evidence: .omo/evidence/mcr-finish-project/task-37-atlas.md; guard commit cc9691cec.

Judgement: internal. The reviewed atlas is produced, hash-pinned and guarded but not served; production activation is openly `NOT_RUN` and tied to the World release path (row 51).

### 41 - QA09 recovery and lifecycle rehearsal

> seven scenarios are measured and pass - outbox replay (READY|1 to DELIVERED|2 with one consumer receipt), baseline pickup, gateway restart (event/giveaways/reservations/outbox ids/receipts/closure rows unchanged), pending pickup under a terminated lock backend, immediate postmaster stop with new PID and intact committed rows, logical backup (783,491 bytes, sha256 bdcf7958...) restored with a post-backup mutation table absent, and partial closure resumed atomically to READ_ONLY|2 with 409 on post-closure writes. Deletion convergence is NOT_RUN as a live saga: with managed deletion configured on both sides, CONCERT_PRIVATE_DATA/SESSIONS/SHARING/TICKETS stay RETRY attempt 3 with last_error Dependency unavailable while FRIEND/NOTIFICATION/POST/PROFILE succeed, so the isolated owner mount PASS is not convergence and is labelled as such.

Judgement: internal with an external dependency symptom. Seven scenarios pass with measured evidence; the deletion saga stalls on `Dependency unavailable`, and the evidence explicitly refuses to present the isolated owner-mount PASS as convergence.

### 42 - QA10 candidate provenance and release checks

> the composition checker and a 51-row task-to-commit trace are delivered for candidate 25b2d356f (109 resolved commits, all ancestors; seven SHAs mentioned in evidence are unmerged and never used as delivering). release-check reports release_decision NOT_ASSESSED with composition_checks INCOMPLETE because no same-candidate owner receipts exist for backend/frontend/service/device/operator/source, the gates BE43/QA01-07/QA09/IN01/IN02/IN04 are NOT_RUN and IN03 BLOCKED, and all 112 task and 61 IA bindings are unbound. The run must be refreshed against the final release SHA; IN05 owns the decision.

Judgement: internal measurement gap. The checker and the 51-row trace exist for an older candidate; owner receipts and gate bindings are unbound. Must be re-run against the final release SHA; IN05 owns the decision.

### 43 - FE24, FE33, FE34

> FE33 (authoritative mural serving/config/adapter) and FE34 (profile preferences with 409 conflict) are closed against real producers with three-width browser evidence. FE24 (protected gift media/motif binding) remains open: BE24 Gateway HTTP media upload/finalize/authorized-read is not composition-qualified for a real caller (scripted probe: metadata 403 PERMISSION_DENIED, content 403, bytes 400 INVALID_INPUT), productionBinding is null and the motif mapping is NOT_READY; GiftDetail truthfully shows the unavailable state and invents nothing.

Judgement: internal. FE33 and FE34 are closed against real producers; FE24 is blocked by BE24 Gateway media composition (403/400 on the scripted probe) and truthfully shows an unavailable state instead of inventing one.

### 45 - FE10 eligibility/legal publication

> no approved FE10 eligibility/legal text supplied. Corrected behaviour: the enrollment endpoint returns 503 POLICY_UNAVAILABLE without a publication, while 422 ACKNOWLEDGEMENTS_REQUIRED applies to the retired signup route and to missing/extra acknowledgements once a policy exists. Binding map and exact missing inputs in .omo/evidence/mcr-finish-project/task-45-fe10.md; only a productionApproval=false dev publication exists and it is never legal approval.

Judgement: genuine external input (no approved legal/eligibility text). Behaviour is correct: `503 POLICY_UNAVAILABLE` without a publication, and the dev publication is marked `productionApproval=false`.

### 51 - closure producer and archive endpoint

> live close -> /archive 200 is unreachable because FREEZE_MURAL..CLEANUP have no composed LifecycleOwners, so a closure job stops at READ_ONLY and both authenticated and anonymous /archive reads truthfully return 404 RESOURCE_UNAVAILABLE (PgPublicArchive::page requires ARCHIVED). Proven live: operator closure 200, member 403 PERMISSION_DENIED, post-closure write 409 INVALID_TRANSITION, forged/expired 401, OPERATOR|1| and READ_ONLY|2 persisted, deny-by-default settlement gate with exactly one early service, nine gate tests, merged-tree PG lifecycle PASS with the PUBLIC projection proven on real PostgreSQL.

Judgement: internal. The operator closure path, authorization, 409s and persistence are proven live; the `/archive` read stays truthfully 404 because `FREEZE_MURAL..CLEANUP` have no composed `LifecycleOwners`.

## Ancestry verification

- 47 distinct delivering commits across the 48 IN05 rows that carry one (rows 35 and 45 share `39a1b4e04`): every `git rev-parse` resolved and every `git merge-base --is-ancestor <sha> 33141de3f` exited 0.
- Rows with no IN05 delivering commit were resolved separately and also pass: row 11 `d962c7d2f` (+ `39512c69a`, `53d547059`), row 47 `7bbcba836` (content) and `ef02302d4` (plan closure), row 48 `bbf9da478` (content) and `33141de3f` (plan closure).
- Row 4 claims no commit (`Commit: N`); nothing to test.
- Evidence files cite seven SHAs that are **not** ancestors of the base: `7c563cf09f`, `5de898eb34`, `caee1a70f8`, `5db23e373b`, `11be1d4395`, `e682b50346`, `a91ed08668`. This is exactly the disclosure in row 42's gap ("seven SHAs mentioned in evidence are unmerged and never used as delivering"); none of them is used as a row's delivering commit, so the count of seven matches the recorded claim.

No ancestry test fails.

## Discrepancies

1. **IN05 snapshot is stale for rows 47 and 48.** `task-47-in05.json` was generated at candidate `7d14251bb` with counts 35 DONE / 14 GAP / 7 OPEN, listing 47 and 48 as OPEN. At the base both are `[x]` with existing evidence and ancestor commits. Current counts: 37 DONE / 14 GAP / 5 OPEN (F1-F5). Not a compliance violation; the reconciliation must be refreshed by F5.
2. **IN05 assigns a delivering commit to an OPEN row.** Row F1 in the ledger is OPEN with no evidence yet it carries `delivering_commit 8352b988`, which is an unrelated `docs(qa)` commit (an ancestor, so harmless to this audit). Ledger artifact, not a plan violation.
3. **Gap formatting for rows 11, 14 and 16.** Their gap is a title parenthetical plus a `Scope result` line rather than a dated `Gap (2026-11-06):` block. The named-gap requirement is met in substance. Row 11 also has no `delivering_commit` in the IN05 ledger despite its evidence recording `d962c7d2f`, `39512c69a` and `53d547059` (all ancestors).
4. **Plan `Commit:` messages diverge from delivered subjects on 23 of 51 rows.** The planned message was not the message actually used (for example row 7's planned `feat(concert/relationship): adopt canonical block policy across consumers` versus the delivered `test(be37): add cfg(test) six-entry consumer visibility probes`). Actual commits are recorded in the evidence and the IN05 ledger; no row is untraceable, but the plan text is not a commit index.
5. **Under-claiming on rows 11, 14 and 16.** Their named gaps have since been delivered by rows 49, 50 and 51 respectively, yet the rows remain `[~]`. This is conservative and is not a discrepancy against completion, but a fresh reconciliation should decide whether to close them.
6. **F2-F5 artifacts are absent.** Expected: the wave has not run. Only F1 is in scope here.
