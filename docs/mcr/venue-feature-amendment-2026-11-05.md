# Venue feature amendment — 2026-11-05

## Authority and current accounting

The user supplied ten Paradise City venue/interaction references in the Frontend King conversation and explicitly required the affected functions to be rebuilt, reflected in Progressions, and repeatedly compared one-to-one against implementation captures. This is an authorized functional amendment, not merely a new map skin.

The active flow specification is [Progressions](progressions.md). Its UI lead owns the [original reference manifest](design/venue-redesign-2026-11-05/reference-manifest.json) and [reference-state map](design/venue-redesign-2026-11-05/reference-state-map.json). This document records scope, migration, ownership and completion boundaries; [progress.md](progress.md) and the [machine register](task-progress-2026-11-04.json) remain the numerical authority.

All112 original task IDs, all61 original IA entries and their original definitions/history remain. The last reconciled original baseline is52/111 active functional outcomes,59 remaining; QA08 is deferred, not passed. The eight mandatory VENU deliverables below are **additional amendment scope**, initially0/8 complete. They are not silently added to the112-task denominator, counted as old completed functionality, or combined into a product-readiness percentage. All must be delivered before the expanded product can be complete. Original acceptance is reopened only when an owner identifies an actual changed/failing original criterion.

## Reference precedence

1. The new venue photographs govern the affected Paradise City venue, Map/Friends/Notes, fixed-landmark invitations, contextual messages and place-note states.
2. Photo5 is the **provisional** base artwork while the Frontend King holds the single optional human choice: Photo5's simplified queues/TOILETS versus Photos1/4's fenced variant. Photo9 supplies provisional Friends anatomy, Photo10 Notes, and Photo2 meeting anatomy. Do not combine conflicting base geometry or label the provisional choice user-approved.
3. Photos1/4/5 are base-map variants; Photo2 covers invitations/contextual MESSAGE; Photos3/6/10 cover notes/list/composer/audience/expiry/replies; Photos7/8/9 cover friends/clusters/freshness/check-in/request/pause. The UI-owned state map provides each precise panel/case/ROI.
4. Burgundy active tabs in these venue references are a local visual override. Other application surfaces retain the selected Parade Ink direction and independent REVENGE/PARADE/DANGER World eras.
5. Preserve every supplied original byte for byte and retain the approved character pack. No decorative left accents, legacy character fallback or screenshot-only UI substitute.

The ten images total3,424,474 bytes. They are product/art references, not proof of calibrated GPS, physical attendance, safe access, current closures or event-day operating approval. Prior Culture Park/Mapv3 adoption and roof-only goals no longer define acceptance for the affected venue view. Useful code, qualified historical evidence, original World obligations and all immutable history remain preserved.

## Required product flows

**Venue overview and Friends.** Implement the full site plan with landmarks, zoom, recenter and MAP/FRIENDS/NOTES tabs. Device-location permission, sharing consent, allowed viewers, explicit check-in and virtual World presence are distinct. Show current, check-in and last-known timestamps truthfully; support authorized friend clusters, selection, location request, pause and revocation, including denied permission, stale data and offline recovery. A location request does not grant access. A friend relationship, shared handle or party membership does not silently authorize GPS disclosure. Domain owners must freeze the actual friendship/permission source and freshness/retention semantics; paused data must never appear current.

**Fixed-landmark invitations and contextual messages.** INVITE actions require an authorized fixed physical landmark and its current review/closure/revision data. Implement incoming/outgoing invitations, accept, decline and cancel. MESSAGE separately opens from an authorized selected friend and event context without a landmark or invitation prerequisite. Landmark and invitation linkage are optional context and never grant message permission; invitationId is optional, and opening itself performs no invite/send/write. There is no global bare-user DM entry. Cancellation rights remain producer-authorized. Historical no-DM/optional-feature assumptions cannot veto this specifically authorized conversation. This does not silently authorize unrelated unrestricted messaging. Invitation acceptance and map/World movement never assert physical arrival or grant location access. Unknown outcomes reconcile through the real operation's receipt rather than repeating a mutation.

**Place notes.** Implement map clusters, list, selection, creation, replies and Meet Here. Notes bind a real place and explicit Friends/Everyone audience;1h and endday expiry are enforced by server time. Define the event/audience boundary and event timezone in the frozen contract; photo examples are not literal timestamps, users or deadlines. Recheck audience, place, block/suspension/deletion, expiry and moderation across detail, replies, clusters, links and caches. Meet Here enters the actual invitation flow, not a fabricated arrival or inert button.

**Client efficiency.** Reuse versioned static site assets, selected-character rendering and local geometry/cluster computation where appropriate. Avoid duplicate per-component requests. Private location/conversation/note state remains scoped by actor/event and invalidated on relevant revocation, expiry, logout and account changes. Client caching never substitutes for server authorization or persists unauthorized location history. The deferred200-user qualification remains separate from these functional invariants.

## Eight required amendment deliverables

| Amendment ID | Required deliverable | Lane and proposed route |
| --- | --- | --- |
| VENU-ART01 | Complete venue artwork and independent site model | ART / FE model; 231 venue artwork; 26e pure physical-venue model |
| VENU-BE-L01 | Location consent, friendship eligibility and freshness | BE; 16e proposed by Root; acceptance/custody pending |
| VENU-FE-L01 | Venue Map and Friends client | FE; 40fde proposed by Root for Map/Friends/Notes shell; 26e model counterpart |
| VENU-BE-M01 | Landmark invitations and contextual conversation | BE; b91d proposed; compatibility/custody pending |
| VENU-FE-M01 | Invitation and meeting-conversation client | FE; 204 proposed by Community; custody pending |
| VENU-BE-N01 | Place-anchored notes and replies | BE; 213 backend Reports/authorization developer proposed; not frontend Profile213 |
| VENU-FE-N01 | Notes map, list, composer and reply client | FE; 40fde Map shell; exact Notes leaf writer must be assigned without overlap |
| VENU-IN01 | Integrated venue flows and repeated reference comparison | Existing implementation owners / integration; Root1fc andRoot153 with each responsible FE/BE/art developer |

Each accepted assignment must have a concrete unbudgeted delivery objective covering implementation, relevant developer-owned checks/fixes, minimum documentation and a committed handoff to the standing publisher. Reuse an aligned unfinished goal; preserve incompatible historical goals truthfully. No false completion/reset/pause, model override or new chat merely to bypass metadata.

## Original-task and IA relationships

Original mappings are continuity links, not assertions that new features already exist. W02/W03/W07/W08 relate to the venue overview, places, explicit check-in and help; W01/W04/W05 retain virtual World/rooms/nearby semantics. M01–M06/M09 relate to existing meeting/profile interfaces; F01–F04 relate to reusable content UI; U05/U08/U09/U10 relate to request/block/account/inbox lifecycle. A shared label or existing beacon does not implement the new invitation, conversation, Everyone-note or phone-location protocols.

The machine register adds ten **extension IA states**, separate from the61 originals: venue overview; Friends list/clusters; location consent/status controls; selected friend/request; fixed-place invitation composer; invitation decisions/status; meeting conversation; notes map/list; note composer/audience/expiry; note/replies/Meet Here. They are view/state contracts, not premature new URL grants. Root153 maps actual route/composition ownership; existing61 routes are neither removed nor silently redefined.

Additive public-wire/schema/generated-type changes belong to the actual contract owner through explicit Root1fc/Root153 transfers. Freeze enough interface shape and deterministic fixtures for independent frontend work; actual integration requires actual producers. Do not use the existing beacon states, catalog revision, handle-sharing grant or generic post endpoint as an undocumented substitute.

## Literal technical custody

Frontend King385 returned any retained additive venue protocol custody before writes. Root1fc now coordinates only these NEW disjoint families: `backend/proto/venue/v1/**`, `backend/docs/api/paradise-city-v1/**`, and `docs/mcr/contracts/paradise-city-v1/**`. It may delegate exact L/M/N child files after actual former-owner STOP, receiving acceptance and current resource booking.

Root153 owns NEW `frontend/src/shared/api/paradise-city-v1/**` generated public types/client mapping as a separate frontend patch. Backend writers never write those frontend files. Developer40fde remains the feature capability-type owner; those capabilities consume the actual frozen wire contract rather than defining a competing protocol.

Existing monolithic API files, generated domain families, manifests, lockfiles, CI and old proto families are not globally transferred. A needed venue-only registration/reexport/build hunk requires an exact transfer from its current writer through the responsible Root. These are source-custody requirements, not a King correctness or acknowledgment gate. No implementation is credited merely because parent custody was granted.

## Preserve and retarget current work

The Frontend King reports actual old-scope stops:231 returned the four old roof-v4 leases with jobs FULL/resources0;26e stopped never-started old Mapv3 adoption while retaining independent PI01 work;40fde stopped the old PI11 venue scope and preserved source03 evidence, commits and its own control/doc changes. Old incompatible native offers were declined. These are actual stop/retarget facts, not goal completion or permission to delete work.

New art/model/frontend partitions are proposals until their exact grants, exclusive existing checkout and receiving acceptance exist. Proposed roles:231 new venue art/anchors;26e independent physical-venue site model/projection;40fde Map/Friends/Notes shell and typed ports;204 invitation/conversation leaf. Root proposes backend16e for location, b91d for invitations/conversation and backend213 for notes, subject to compatibility and actual custody. Protected205/Party/Gift and unrelated runtime work are not presumed available. The backend213 proposal is not the frontend Profile213 writer.

Use the same existing context when compatible. Preserve useful pan/zoom, interaction and source/test work with provenance. Do not reset or rebase peer worktrees, publish obsolete venue work as new-target acceptance, infer release from a stopped/failed chat, or acquire shared routes/types implicitly. A source/contract owner and sole allocator21d handle real transfers and usable resources directly; no Main acknowledgment is required.

Main owns the four central management documents and this amendment. UI533 owns Progressions and the new reference directory after Main's literal STOP before any writes to those overlapping paths. This resolves the crossed documentation handoff; it does not create a second writer. Publisher412e retains sole normal non-force master publication.

## Ongoing developer-owned comparison

For every mapped state and each affected iteration, the implementation owner records:
- reference photo SHA and exact panel/ROI/case identifier;
- actual viewport/DPR, source commit/build identity, capture time and service/fixture conditions;
- the matching actual application capture, normalized pair and overlay/diff;
- concrete fixed-region discrepancies, the correction and subsequent owner disposition.

Compare overview, fresh/stale friends, clusters, selected friend/request/pause, fixed meeting and invitation decisions, note clusters/list/selection, composer/audience/expiry/reply and related empty/denied/offline states. Preserve original360/390/430 and applicable short-height/focus/safe-area/device obligations. Variable-data masks may cover named authorized content only; they cannot conceal geometry, typography, controls, missing states or entire panels.

Material fixed-region differences remain open. Controlled visual fixtures do not certify actual GPS, authorization, persistence, real two-client flows or physical devices. Conversely, do not add blanket live-service prerequisites to an original pure-client criterion. Developers inspect and fix their captures; Kings/SLs do not perform code, query, runtime or pixel reviews. One publisher receipt establishes delivery; no review/ACK/proof-only successor is required.

## Inputs and completion order

The supplied venue name is Paradise City and these ten visual references are now received. Update the existing physical-place request accordingly; do not ask the user again for those supplied inputs. Still unresolved are the genuinely required authoritative georeferencing/calibration, current permitted meeting locations/access/closure reviewer and validity, and any third-party physical-source rights not established by supplied art. Only dependent physical claims wait for those inputs. UI/common-flow/contract work continues.

The Frontend King alone holds the optional base-variant question. Root/domain owners first trace existing sources and references before escalating any missing product decision about friendship, location freshness, legitimate check-in, conversation access/retention or endday timezone. Do not invent those values or duplicate existing human questions.

Proceed in parallel: preserve/stop incompatible work; freeze explicit contract seams and site model; implement disjoint art, location, meeting and note clients/producers through existing owners; integrate each usable producer as it arrives; repeat developer comparison and functional fixes; then reconcile the original112/61 **and** these required extension deliverables/states for IN05. Unrelated work never waits for a full venue/backend wave or this management document.
