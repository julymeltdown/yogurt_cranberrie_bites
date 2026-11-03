# FE13 avatar editor

Status: **IN_PROGRESS**. Source base `9f72a71a99091fe89411b180af3404be85d963f1`.
Owned paths: `frontend/src/features/avatar/**` and this document only. No copied
Bigyeting UI or renderer code, backend imports, source-art changes or dependency
changes. C03 `contracts/core/v1/operations.json` and core fixture shapes are the
HTTP baseline; C06 AvatarConfig v1 remains unchanged.

`createAvatarEditorController({auth, createPreview, onCommitted?})` uses the
canonical session's `privacy` and `request` methods. `AvatarEditor` is route-ready
and owns start/stop while mounted; the composition owner finally disposes the
controller. Strict effect stop/start creates a fresh private lifetime.
`createPreview(scope, signal)` creates a scope-owned lease; the forthcoming FE12
adapter uses the committed shared compositor. Tests inject an explicitly
synthetic port. No production artwork is claimed.

GET `/api/concert/events/{eventId}/me` verifies the event and canonical participant
and profile identities before reading `profile.avatar`. PUT `/me/avatar` submits
only `{expected_revision, config}`. Save success must return a strictly newer
revision and the submitted config. Drafts and preview choices never publish an
optimistic world outfit. `onCommitted(avatar, scope)` is the integration seam for
world/profile updates or invalidation using the same verified response. Root
wiring remains delivery-owned and is not included in this patch.

Presets/items/tones come only from the current immutable catalog. Unknown IDs,
wrong slots, unsupported tones and source-sheet IDs are unavailable. Failed
saves keep drafts. VERSION_CONFLICT and uncertain responses block writes until
an explicit latest-state read and user reconciliation; choosing "Keep my draft"
rebases the next explicit write, without automatically replaying it. Cancel
restores the saved config; it cannot claim that an uncertain write was undone.
A pending dispatched save cannot be cancelled as though it had no server effect.
Privacy, identity and event changes abort operations, erase private state and
dispose all owned preview resources. Late results cannot reattach previews or
publish old-user saves.

Initial verification (2026-11-04): Node 24.22.2, retained pnpm 11.7.0 offline frozen
install exit 0; TypeScript strict check exit 0; focused controller suite **15/15**
exit 0. Covers draft transitions, valid/unknown selection, failed save, conflict,
uncertain/malformed response, explicit reconciliation, stale read/identity,
single-flight save, cancel, policy teardown, late preview release and remount.
Browser/UI verification and committed FE12 adapter are still in progress.
Real service, production art, world/profile propagation, device and IN01 gates
are **NOT_RUN**. This implementation does not accept FE13 or AR03/IN01.
