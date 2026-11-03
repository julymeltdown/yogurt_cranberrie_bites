# FE04 image upload and authorized preview handoff

Status: **MOCK_READY** for the bounded headless implementation. No BE24 backend,
real media integration, real device, IN02/IN03 or release acceptance is claimed.

## Candidate and ownership

- Worktree: `/Users/lhs/Desktop/mcr-worktrees/fe-media-upload`.
- Branch: `codex/mcr-fe-media-upload`.
- Assignment base: `48458d597af73bd695c8d5813b123c6a96e55321`.
- Canonical binary extension: `4d65ed47a29ac6fddf301359c068d69bcb80419f`.
  Delivery reports its separate integration as `a56f043`. The shared HTTP files
  were explicitly transferred to this worker and subsequently released back to
  central ownership. This document does not certify remote publication.
- Media implementation: `795f46b4a32ab26bfb0e1cb334d46b6c69ed9b5a`, child of
  the binary extension. The following documentation/diagnostic-only commit is
  identified by `git log -1 --format=%H -- docs/mcr/frontend/media-upload.md`.
- Only the designated delivery integrator publishes master. Worker handoffs go
  directly to delivery and consumers; main/SL review is not a prerequisite.

The binary commit changes exactly:

```text
frontend/src/shared/api/http.ts
frontend/src/shared/api/__tests__/http.test.ts
```

The feature commit changes exactly:

```text
frontend/src/shared/media/types.ts
frontend/src/shared/media/image-policy.ts
frontend/src/shared/media/image-compression.worker.ts
frontend/src/shared/media/prepare-image.ts
frontend/src/shared/media/preview.ts
frontend/src/shared/media/contract.ts
frontend/src/shared/media/transport.ts
frontend/src/shared/media/read-media.ts
frontend/src/shared/media/presigned-upload.ts
frontend/src/features/media/model/upload-controller.ts
frontend/src/features/media/model/__tests__/fixtures.ts
frontend/src/features/media/model/__tests__/controller.test.ts
frontend/src/features/media/model/__tests__/policy.test.ts
frontend/src/features/media/model/__tests__/preparation.test.ts
frontend/src/features/media/model/__tests__/transport.test.ts
frontend/src/features/media/model/__tests__/read-media.test.ts
frontend/src/features/media/model/__tests__/browser-fixture.html
frontend/src/features/media/model/__tests__/browser-fixture.ts
frontend/src/features/media/model/__tests__/browser-smoke.mjs
frontend/src/features/media/model/__tests__/baseline-diagnostic.mjs
```

The follow-up changes only this document and the diagnostic script's assertion
that its baseline alias was exercised. No source archive, supplied artwork,
backend, contract, manifest, lockfile, router or production UI was changed.

## Provenance and contract pin

Read the root/frontend working agreements, plan FE04/BE24, project assessment,
execution plan, parallel-delivery authority and C01 source-audit/reuse inventory.
The audited Bigyeting candidate is `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`.
All production modules and tests here are newly authored; no upstream module,
rendering, endpoint assumptions or mixed image/video barrel was copied. The
baseline diagnostic temporarily reads earlier **MCR** transport code, rewrites
only its local import paths and removes that test-only file in `finally`.

Runtime dependencies are the retained Zod 4.4.3 and browser-image-compression
2.0.2 plus the canonical FE01/FE09 HTTP/session interfaces. No package change,
external CDN worker, video/FFmpeg, new token store or separate authenticated
client exists.

C04 remains a **REVIEW_CANDIDATE**. The bounded implementation pins these files at
its assignment base; `feed.schema.json` last changed at
`d0649ec2a1ea8dd520627fc214792c877d1d4c22`:

| Input | SHA-256 |
|---|---|
| `contracts/domain/v1/feed.schema.json` | `9c8d991df0bb707d7a7a2d43d4b132e7fadb878a52633e1c1f338afc9407f8ed` |
| `contracts/domain/v1/policy.json` | `508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe` |
| `contracts/domain/v1/operations.json` | `42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223` |
| Prior `frontend/src/shared/api/http.ts` | `f214c64de92a14f49d23c764e1c950d87dfc8e43e9a2e79cd5d4ac56915d1f3f` |

C04 initializes a private PENDING media intent with an **authenticated same-origin
PUT path**, followed by an authenticated byte command and finalize. Each command
uses a distinct original canonical UUID idempotency key. The bytes carry only the
prepared image; init declares purpose, MIME, length and SHA-256; finalize carries
the UPLOADED revision. Authenticated calls use `SessionController.request`, its
canonical identity, browser intent, no-store, error qualification and cancellation.
No invented presigned endpoint or upload header fields were added.

The independent `uploadPresignedImage` helper is **unused by C04**. It accepts an
explicit HTTPS origin allowlist and raw Blob only, sends `credentials: omit`, only
Content-Type, no referrer, and errors on redirects. It has no session port or
caller-header option. It cannot mark an attachment READY. A future presigned
contract must separately qualify its exact hosts, paths, signed fields and
expiration before choosing this mode.

## Consumer interfaces

```ts
import {
  createImageUploadController,
  createMediaUploadTransport,
} from './frontend/src/features/media/model/upload-controller'
import { createMediaPreviewController } from './frontend/src/shared/media/read-media'

const media = createImageUploadController({
  session: auth.session,
  transport: createMediaUploadTransport(auth.request),
  purpose: 'POST', // or GIVEAWAY / REPORT
})
const stopPrivacy = auth.privacy.registerCleanup(() => media.cancel())
// Consumer cleanup: stopPrivacy(); media.dispose().
```

`ImageUploadPort` exposes `read`, `subscribe(listener: () => void)`,
`select(file)`, `upload`, `recover`, `retry`, `cancel` and `dispose`. Select only
prepares an image; uploading is explicit. `UploadState` has `phase`, canonical
`scope` (generation/userId/eventId), `previewUrl`, current-stage `progress`,
`attachment` and a safe `error` code. All optional state uses null. No credentials,
raw errors, filename or signed URL appear in this state. Phases are idle,
preparing, prepared, initializing, uploading, finalizing, ready, failed, uncertain
and cancelled. Subscribe does not invoke the listener immediately.

Only phase ready exposes `attachment: ReadyMedia`; all other phases expose null.
The attachment has exactly C04 `media_id`, `state: READY`, revision, width, height,
content_type, purpose and same-event `read_path`. Consumers must additionally bind
it to their current privacy scope and intended purpose before submitting its ID.
No meeting-photo purpose or MeetingDescription attachment field exists.

`createMediaPreviewController({session, request: auth.request, mediaId, purpose})`
exposes `read`, `subscribe`, explicit `load`, `cancel` and `dispose`. Its state has
phase idle/loading/ready/failed/cancelled, scope, previewUrl, media and safe error.
It reads authorized metadata, then bounded binary content through the canonical
HTTP request; it verifies actual image headers against the returned MIME and
dimensions before creating an owned Blob URL. Render only that ephemeral URL;
never assign a protected API URL to an img. Register privacy cleanup and dispose
on unmount. Identity/event invalidation automatically clears and revokes both
controllers' resources. FE28 and community consumers received these interfaces
early; root wiring remains delivery-owned.

## Lifecycle and failure behavior

- Reject empty/>10MiB inputs, unapproved MIME/signature, animation, dimensions
  above 4096, and pixel count above 16,000,000 **before** browser decode. PNG,
  ordinary JPEG and WebP header checks are a client gate, not full decoder or
  server-security certification.
- Compress/re-encode in an owned bundled same-origin module worker with
  browser-image-compression, target 2048px/2MiB and WebP; preserveExif is false.
  Recheck actual output MIME/bytes/dimensions and compute its SHA-256. Target size
  is a compression preference; the 10MiB contract limit is enforced afterward.
- The worker uses no nested library worker or CDN. Completion/error/abort and its
  30-second stall deadline clear handlers/timers and terminate it. Cancellation
  revokes preview URLs, aborts requests and releases identity/file/Blob/key state.
  Generation fences reject noncooperative late completions. The canonical HTTP
  layer also cancels late response streams.
- Progress reflects the current stage. Compression has measured progress;
  canonical Fetch byte upload uses null because transferred bytes are unknown.
- 202, malformed/lost finalize, network error and timeout never imply uploaded or
  READY. A successful PUT alone never attaches the media.
- Recovery performs one explicit status read bound to actor/event/operation/key
  and known media ID. A SUCCEEDED/200 receipt still requires a fresh authorized
  READY media read with matching ID/purpose/path and adequate revision. Pending,
  retired, expired, inaccessible and failed receipts cannot authorize replay.
- `retry` only retries failed local preparation before a server mutation. Current
  C04 enables no reviewed mutation replay policy. No automatic mutation retry,
  new-key recovery, background poll, draft persistence or presumed 404 success
  occurs. Cancel abandons local state; it does not certify server deletion.
- Client checks do not replace BE24 ownership, byte/hash validation, single-frame
  decode/re-encode, metadata removal, authorization or malware defenses.

## Executed evidence and limitations

Pinned runtime: Node 24.22.2, pnpm 11.7.0, TS 6.0.3, Vitest 4.1.9, Vite 8.1.3,
Oxlint 1.72.0. Node/pnpm live under `/Users/lhs/Desktop/mcr-tools`. Commands below
run in this worktree's `frontend/` with that Node directory prepended to PATH.

| Command / check | Result |
|---|---|
| `pnpm install --frozen-lockfile --offline` | Exit 0; retained resolutions, no lock edit. |
| `node node_modules/vitest/vitest.mjs run src/features/media/model/__tests__` | Exit 0; **73 tests in five files**: MIME/bytes/dimensions, compression errors, worker cancel/stall cleanup, all upload-stage cancels, stale identities, explicit retry restrictions, receipt recovery, canonical transport and authorized media display. |
| `node node_modules/vitest/vitest.mjs run src/shared/api/__tests__ src/features/auth/__tests__` at binary checkpoint | Exit 0; **413 tests**. |
| Combined media/HTTP/auth before adding display/worker cases | Exit 0; 476 tests. |
| Final combined media/HTTP/auth (486 cases) | **Exit 1; 485 pass, one enrollment cancellation assertion fails**, described below. This is not a fully green broad gate. |
| `pnpm build` | Exit 0, includes typecheck. Existing main/community >500kB chunk warnings persist; media is not yet wired into the application root. |
| `node node_modules/oxlint/bin/oxlint src vite.config.ts` | Exit 0. |
| `node src/features/media/model/__tests__/browser-smoke.mjs` | Exit 0; installed Chrome **156.0.8078.80**, development and separately built production fixture. Bundled module-worker startup is exercised. |
| Root `python3 scripts/mcr/check-plan.py` | Exit 0; 13 archive checksums match; planning/provenance only. |
| Root `node scripts/mcr/check-ui-origin.mjs` | Exit 0; no legacy rendering/assets/copied-source exception. |
| Root `python3 scripts/mcr/check-boundaries.py --base 48458d597af73bd695c8d5813b123c6a96e55321 --lane frontend --allow-doc docs/mcr/frontend/media-upload.md` | Exit 0; exact assigned paths and frontend imports manually checked. |
| `git diff --check`; protected source/manifest/lock diff against assignment base | Exit 0; no protected edits. |

Native fixture results in **both modes**: synthetic JPEG 34,717 bytes at
3200×1800 becomes WebP **4,752 bytes at 2048×1152**; metadata marker absent, SHA-256
matches, preview decodes and becomes unfetchable after revocation, peak owned
workers one and final live workers zero. Worker abort settles as aborted with
zero live workers. An HTTPS presigned route fixture with a preexisting cookie
receives no Cookie, Authorization, browser-intent, idempotency or client headers.
Raw reproducible report: `/tmp/mcr-fe04-evidence/results.json`. These are synthetic
fixtures only; browser rendering and codec support on actual phones are NOT_RUN.

The final broad run exposed
`src/features/auth/__tests__/enrollment.test.ts:131`: a stored cloned Request's
signal sometimes remains un-aborted after logout. Earlier broad runs passed;
a single focused test passed; later full-file runs failed. The diagnostic
`node src/features/media/model/__tests__/baseline-diagnostic.mjs` reproduces the
same **48 pass / 1 fail, exit 1** using the exact pre-FE04 transport at the
assignment base. It verifies its baseline alias is used and removes its temporary
file. No auth test/source or released HTTP file was changed to suppress this
failure. Delivery has the finding for the auth owner; this evidence does not
establish the root cause or certify the broad auth gate.

BE24 production handlers, live storage, real multiuser image publication,
feed/giveaway root integration, revocation delivery from a real service, actual
iOS/Android, load/security/release qualification and remote ancestry of the media
commit are **NOT_RUN** in this worker. The real adapter fails through actual
backend responses; deterministic success exists only in test modules. No task,
IA or release acceptance count changes here.
