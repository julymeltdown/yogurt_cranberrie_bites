# FE04 / FE28 media consumer privacy review

Date: 2026-11-04 (Asia/Seoul). Task: bounded independent media/privacy QA.
Assigned worktree: `/Users/lhs/Desktop/mcr-worktrees/qa-media-consumer`.
Branch: `codex/mcr-qa-media-consumer`.
Sole delivered path: `docs/mcr/reviews/media-consumer-privacy.md`.

## Result

No new P1/P2 defect was reproduced within this bounded review. Independent
cross-feature probes passed **13/13 canonical transport/controller cases**,
**8/8 component guard cases**, and **15 native browser scenarios in two batches**.
Late responses did not restore a READY attachment or blob preview after tested
cleanup. A successful uncertain-finalize receipt required a fresh authorized
READY read before FE28 could attach its media ID. This is deterministic fixture
and development-browser evidence, not FE04/FE28/BE24 or IN03 acceptance.

No application, shared transport, contract, manifest, lockfile, existing test,
source archive or artwork was edited. No Cargo/PostgreSQL/build target was used.
The previous FE28 receipt/cache probes and broad suites were not rerun.

## Candidate and source provenance

- Assigned and executed candidate: `5c95ed4ddb243853b27fb625f726341b9771dadd`.
- Canonical binary transport integration: `a56f043f9d059cfc98e68278bb2119a6ca20f9ef`.
- FE04 controller/reader implementation: `095214956d00bda8ce5990ade4b9ae8f309323ec`.
  Scoped `git diff --exit-code` against original `795f46b4a32ab26bfb0e1cb334d46b6c69ed9b5a`
  confirms the media implementation trees match (exit 0).
- FE28 final media consumer evidence: original `697b3ec3cfdad69b1f1eda86191cb4808726a270`,
  integrated `a413e69f6864d3a5578fb3149507921e96e9c5b5`. Scoped feature-tree diff
  exits 0. That owner's earlier browser report is context, not this QA's proof.
- Prior receipt/privacy report is integrated as `b392f56889c880f2fc5b9f2d92c0c41b318fdc73`;
  its findings are not reopened or requalified here.
- Reviewed FE04/FE28/BE24 requirements, project assessment, execution plan and
  parallel-delivery authority. C04 remains the pinned review candidate; this QA
  changes no contract or acceptance decision. All probe code is newly authored
  test-only code; no Bigyeting rendering or production source was copied.

SHA-256 of the executed inputs:

| Path | SHA-256 |
|---|---|
| `frontend/src/shared/api/http.ts` | `441983a9e40cea38ad2f93a5a7101ee22a0d10dc3f50ec9b6ecf878dcd4c70a1` |
| `frontend/src/shared/media/read-media.ts` | `a91aa13d5a3d074985e50ea2252728ef17b526376c798171138f8bd63f802cd8` |
| `frontend/src/shared/media/prepare-image.ts` | `a1ac4b47bdc744afd1065e82bc950a83c0d01c3d7946e098e0e6adb6171db128` |
| `frontend/src/features/media/model/upload-controller.ts` | `e187899b6e8c4c22956eb1f1849420265603d80d56699c2a0baff76334f3e571` |
| `frontend/src/features/event-feed/FeedImage.tsx` | `938f5201bf5df64f655afb830de1e2736f8907564fc313eec493062722d7ed48` |
| `frontend/src/features/feed-thread/controller.ts` | `712d7b6449c4cb4a263f42fb3ccc7ac04553221ffceb42569e9d4e871c2efa22` |
| `contracts/domain/v1/feed.schema.json` | `9c8d991df0bb707d7a7a2d43d4b132e7fadb878a52633e1c1f338afc9407f8ed` |
| `contracts/generated/mcr-domain.openapi.json` | `c47e767505e660d8f5eb69bb9759eae1f0aaee6472e4fb963edc82e060385f51` |

## Independent controller and component evidence

The 13 cross-feature cases instantiate actual `createSessionController`,
`createMediaUploadTransport(auth.request)`, `createImageUploadController` and
`createFeedThreadController({ auth, media, access })`. The fixture uses only an
in-memory test credential, checks canonical bearer/no-store requests, and injects
controlled same-origin fetch responses. Its deterministic image preparer isolates
response timing; real compression is covered separately in the browser.

| Cases | Observed result |
|---|---|
| Late finalize after explicit cancel, logout, account replacement, event replacement, or policy bump (5) | Request signal aborted; local blob revoked once; scope/attachment/preview cleared; late result cannot attach to FE28 |
| Noncooperative late metadata or binary fetch after policy cleanup (2) | Request aborted; late response stream cancelled; no blob URL created or media retained |
| Lost finalize response followed by explicit receipt status and fresh metadata: valid, denied, stale revision, wrong purpose, or untrusted read path (5) | Exactly POST/PUT/POST/GET/GET, no mutation replay; only valid authorized fresh READY metadata can attach; the other four remain uncertain |
| Policy bump while successful-receipt recovery awaits fresh READY metadata (1) | Recovery cancelled; late READY does not restore attachment or FE28 draft IDs |

Eight jsdom cases exercise the `FeedImage` structural consumer boundary with an
explicitly injected READY-state port: protected API src, remote src, foreign user,
foreign event, stale session generation, wrong media ID, wrong purpose and wrong
read path. All render **no img/src element**. These are component rejection
probes, not claims of server authorization or alternate real transport coverage.

## Independent native browser evidence

Installed Chrome **156.0.8078.80**, viewport **390×844**, React StrictMode,
Vite development fixture, real FE04 controllers/transport, FE28 PostComposer and
FeedImage, actual bundled module worker and browser Blob URLs. The native server
is explicitly test-only; it requires the fixture bearer and browser-intent/key on
mutations and never accesses a real service. The harness instruments actual
fetch AbortSignals, URL create/revoke and owned Worker lifetimes.

| Native cases | Count | Observed result |
|---|---:|---|
| Held metadata × cancel/logout/account/event/policy/unmount | 6 | Each aborts one observed request, creates zero URLs and leaves no img |
| Partial binary stream (first four bytes delivered, remainder held) × the same six invalidators | 6 | Each aborts one observed request, creates zero URLs and leaves no img |
| Already displayed protected image, then policy cleanup | 1 | Exactly one blob URL created/revoked; DOM img disappears; fetching old URL rejects |
| Actual compression and upload, held finalize, then policy cleanup | 1 | Prepared blob revoked and unfetchable; no READY attachment/draft IDs; peak worker 1, final live workers 0; uploaded WebP 560 bytes |
| Policy cleanup immediately after actual Worker construction, before its result | 1 | Peak worker 1, final live workers 0; no blob URL, attachment or API request |

The first 14 scenarios ran together and the additional active-worker scenario
ran alone after extending the runner. Both batches exited 0 with **zero page
errors**. No combined rerun of all 15 is claimed. The retained runner below
contains both branches; default mode now includes the extra worker scenario.
The worker hook only schedules canonical policy invalidation; it does not replace
worker code or fabricate a successful result. The instrumented constructor still
starts the actual bundled module worker.

The successful rendered image used an owned `blob:` URL, not its authenticated
API path or a remote URL. A real revoked URL became unfetchable. HTTP/body
cancellation was measured in both deterministic noncooperative-fetch probes and
a real browser partial-stream case; neither substitutes for live-server policy
propagation testing.

## Commands, exits and limits

All commands run in the assigned checkout. `pnpm` below is
`node /Users/lhs/Desktop/mcr-tools/pnpm-11.7.0/package/bin/pnpm.cjs`, with
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin` prepended to PATH.
Version pins: Node 24.22.2, pnpm 11.7.0, Vitest 4.1.9, Vite 8.1.3.

| Command | Result |
|---|---|
| `pnpm -C frontend install --offline --frozen-lockfile` | Exit 0; 199 reused, zero downloaded; retained lockfile unchanged |
| `pnpm -C frontend exec vitest run .qa-media/cross-feature.test.ts` | Exit 0; 13/13 |
| `pnpm -C frontend exec vitest run .qa-media/consumer-guards.test.tsx` | Exit 0; 8/8 |
| `pnpm -C frontend exec node .qa-media/browser.mjs` before active-worker extension | Exit 0; 14/14 native scenarios |
| `QA_ONLY_WORKER=1 pnpm -C frontend exec node .qa-media/browser.mjs` after extension | Exit 0; 1/1 additional native scenario |
| `python3 scripts/mcr/check-plan.py` | Exit 0; 13 source archive checksums match; planning/provenance only |
| `git diff --check` | Exit 0 |

One initial temporary test was placed one directory too deep; its import failed
before any test ran (exit 1). The file was moved to the intended temporary path,
the accidental empty directories removed, and the corrected 13-case command
passed. This setup failure is not a product failure.

The actual source relies on **consumer-owned policy cleanup**: FE28 thread
runtime calls `media.cancel()` on privacy cleanup; FeedImage registers a privacy
cleanup callback and disposes its controller on effect cleanup. Low-level media
controllers independently fence session identity/event generations, but do not
subscribe to the privacy coordinator themselves. Future root composition must
retain that documented cleanup registration and one canonical session. This
review qualifies the explicitly composed consumer boundary only.

Source-only inspection finds no media controller/FeedImage composition in
`frontend/src/app` or `frontend/src/main.tsx` at this candidate. Production-root
media wiring, live BE24/storage validation and deletion, real token/policy event
delivery, two-account IN03, actual devices, production-bundle codec/worker behavior,
load/security and release checks are **NOT_RUN**. Header/MIME checks do not prove
server-side full decoding, byte/hash verification or metadata removal. Cancellation
means local resource cleanup, not server upload deletion. No new P1/P2 finding
requires an ownership transfer or application fix in this bounded review.

The unbudgeted goal covers this report, relevant evidence, concrete report/fix
corrections if found, committed handoff and clean stop. The report commit must
descend directly from assigned `5c95ed4`; no application commits are included.
Only delivery publishes master; remote integration of this report is not claimed
before it happens. Full FE04/FE28/IN03 acceptance and counts remain unchanged.

## Reproducible temporary probes

The temporary probe directory is removed before final handoff. The following
source is retained here so the checks can be reconstructed at the pinned candidate
without writing tracked tests or relying on an external temporary artifact.
Create the listed files under `frontend/.qa-media/`, run the commands above, and
remove that directory afterward. These fixtures contain test-only credentials
and deterministic backend responses, and are not application entrypoints.

Temporary artifact SHA-256 values (results are summarized above):

- `cross-feature.test.ts`: `f9de83c1a886210a94ae8e5b2291a28520892fd19e3c85e2ac80fc3a3e6ee833`
- `consumer-guards.test.tsx`: `9d89d210958327fb65b9b15cb0fec52ee5fdeecda599e6d0e8bfbe7c1bb542d7`
- `browser.tsx`: `de97af541662f5dcfd7b54061b9a6296328937e223f01ce6fad40670473d6871`
- `browser.html`: `6c31505ad168d12841456529bc596a0e88c890bf4e142496ec5370bf92883352`
- `browser.mjs`: `62f12f4bf4918c147875ba170922cec8a383b501790e6be82d166f870267461f`
- `browser-results.json`: `95c588d6f55121a0759e3bac633005ac9cf385ac50158021aa2106e50900a177`
- `worker-browser-results.json`: `4fbe7571efcce2126e55215540fd5c76bbbe1d3e7b1b5cb6a69ea4dc30e0a5db`

<details>
<summary>cross-feature.test.ts</summary>

```ts
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createSessionController } from '../src/features/auth/model/session'
import { createImageUploadController, createMediaUploadTransport } from '../src/features/media/model/upload-controller'
import { createMediaPreviewController } from '../src/shared/media/read-media'
import { createFeedThreadController } from '../src/features/feed-thread/controller'
import { bytes, deferred, event, file, image, intent, mediaId, ready, uid, user } from '../src/features/media/model/__tests__/fixtures'
const disposers: (()=>void)[]=[]
beforeEach(()=>vi.stubGlobal('location',{origin:'https://mcr.example'}))
afterEach(()=>{disposers.splice(0).forEach(f=>f());vi.unstubAllGlobals()})
const access={read:()=>({available:true,readOnly:false}),subscribe:()=>()=>{}}
function fixture(handler:(r:Request)=>Response|Promise<Response>) {
 const requests:Request[]=[]
 const auth=createSessionController({eventId:event,fetch:async input=>{const r=input as Request;requests.push(r);expect(r.headers.get('Authorization')).toBe('Bearer fixture-only');expect(r.cache).toBe('no-store');return handler(r)}})
 auth.session.replace(0,{userId:user,eventId:event,accessToken:'fixture-only'})
 const previews={create:vi.fn(()=> 'blob:qa-owned'),revoke:vi.fn()}
 const media=createImageUploadController({session:auth.session,transport:createMediaUploadTransport(auth.request),purpose:'POST',prepare:async()=>image(),previews})
 const thread=createFeedThreadController({auth,access,media});thread.start()
 disposers.push(()=>{thread.dispose();media.dispose();auth.dispose()})
 return {auth,media,thread,previews,requests}
}
const invalidators=['cancel','logout','account','event','policy'] as const
function invalidate(s:ReturnType<typeof fixture>,kind:typeof invalidators[number]) {
 if(kind==='cancel')s.media.cancel()
 else if(kind==='policy')s.auth.privacy.changePolicy(s.auth.privacy.capture(),'block')
 else s.auth.session.replace(s.auth.session.read().generation,kind==='logout'?null:{userId:kind==='account'?uid(91):user,eventId:kind==='event'?uid(92):event,accessToken:'fixture-only'})
}
it.each(invalidators)('late finalization after %s cannot attach and cancels its request/preview',async kind=>{
 const late=deferred<Response>()
 const s=fixture(r=>r.url.endsWith('/uploads')?Response.json(intent()):r.url.endsWith('/bytes')?Response.json({media_id:mediaId,state:'UPLOADED',revision:2}):late.promise)
 await s.media.select(file());const upload=s.media.upload();await vi.waitFor(()=>expect(s.media.read().phase).toBe('finalizing'))
 const request=s.requests.at(-1)!;invalidate(s,kind);await upload
 expect(s.media.read()).toMatchObject({phase:'cancelled',previewUrl:null,attachment:null,scope:null});expect(s.previews.revoke).toHaveBeenCalledExactlyOnceWith('blob:qa-owned')
 expect(request.signal.aborted).toBe(true)
 late.resolve(Response.json(ready()));await Promise.resolve();await Promise.resolve()
 expect(s.thread.attachReadyMedia()).toBe(false);expect(s.thread.read().draft.media_ids).toEqual([]);expect(s.media.read().attachment).toBeNull()
})
it.each(['metadata','binary'] as const)('late %s response after privacy cleanup cannot create blob and releases stream',async stage=>{
 const late=deferred<Response>(),cancel=vi.fn()
 const s=fixture(r=>stage==='metadata'||r.url.endsWith('/content')?late.promise:Response.json(ready()))
 const preview=createMediaPreviewController({session:s.auth.session,request:s.auth.request,mediaId,purpose:'POST',previews:s.previews})
 const stop=s.auth.privacy.registerCleanup(()=>preview.cancel());disposers.push(()=>{stop();preview.dispose()})
 const flight=preview.load();await vi.waitFor(()=>expect(s.requests).toHaveLength(stage==='metadata'?1:2))
 const request=s.requests.at(-1)!;s.auth.privacy.changePolicy(s.auth.privacy.capture(),'block');await flight
 const data=stage==='metadata'?new TextEncoder().encode(JSON.stringify(ready())):bytes()
 late.resolve(new Response(new ReadableStream({start(c){c.enqueue(data)},cancel}),{headers:{'Content-Type':stage==='metadata'?'application/json':'image/png'}}))
 await vi.waitFor(()=>expect(cancel).toHaveBeenCalled())
 expect(request.signal.aborted).toBe(true);expect(preview.read()).toMatchObject({phase:'cancelled',previewUrl:null,media:null});expect(s.previews.create).not.toHaveBeenCalled()
})
it.each(['success','denied','stale','wrong-purpose','wrong-path'] as const)('uncertain finalize receipt requires fresh authorized READY read: %s',async outcome=>{
 let key='',writes=0,read=0;const methods:string[]=[]
 const s=fixture(r=>{
  methods.push(r.method)
  if(r.method!=='GET'){writes++;if(r.url.endsWith('/uploads'))return Response.json(intent());if(r.url.endsWith('/bytes'))return Response.json({media_id:mediaId,state:'UPLOADED',revision:2});key=r.headers.get('Idempotency-Key')!;throw Error('lost-finalize-response')}
  if(r.url.includes('/commands'))return Response.json({receipt_id:uid(80),event_id:event,actor_id:user,operation_id:'finalizeConcertUpload',idempotency_key:key,state:'SUCCEEDED',resource_id:mediaId,resource_revision:3,failure_reason:null,created_at:'2026-11-03T00:00:00Z',expires_at:'2099-01-01T00:00:00Z',result_status:200})
  read++;if(outcome==='denied')return Response.json({code:'DENIED',message:'Safe',details:{reason:'PERMISSION_DENIED'}},{status:403})
  return Response.json({...ready(),...(outcome==='stale'?{revision:2}:outcome==='wrong-purpose'?{purpose:'REPORT'}:outcome==='wrong-path'?{read_path:'https://untrusted.invalid/photo'}:{})})
 })
 await s.media.select(file());await s.media.upload();expect(s.media.read().phase).toBe('uncertain');expect(s.thread.attachReadyMedia()).toBe(false)
 await s.media.recover();expect(writes).toBe(3);expect(read).toBe(1);expect(methods).toEqual(['POST','PUT','POST','GET','GET'])
 if(outcome==='success'){expect(s.media.read().phase).toBe('ready');expect(s.thread.attachReadyMedia()).toBe(true);expect(s.thread.read().draft.media_ids).toEqual([mediaId]);expect(s.media.read().phase).toBe('cancelled');expect(s.previews.revoke).toHaveBeenCalledOnce()}
 else {expect(s.media.read().phase).toBe('uncertain');expect(s.thread.attachReadyMedia()).toBe(false);expect(s.thread.read().draft.media_ids).toEqual([])}
})
it('policy invalidation during fresh READY reread drops recovery and cannot reattach',async()=>{
 const late=deferred<Response>();let key=''
 const s=fixture(r=>{
  if(r.url.endsWith('/uploads'))return Response.json(intent());if(r.url.endsWith('/bytes'))return Response.json({media_id:mediaId,state:'UPLOADED',revision:2})
  if(r.method==='POST'){key=r.headers.get('Idempotency-Key')!;throw Error('lost')}
  if(r.url.includes('/commands'))return Response.json({receipt_id:uid(80),event_id:event,actor_id:user,operation_id:'finalizeConcertUpload',idempotency_key:key,state:'SUCCEEDED',resource_id:mediaId,resource_revision:3,failure_reason:null,created_at:'2026-11-03T00:00:00Z',expires_at:'2099-01-01T00:00:00Z',result_status:200})
  return late.promise
 })
 await s.media.select(file());await s.media.upload();const recovery=s.media.recover();await vi.waitFor(()=>expect(s.requests).toHaveLength(5))
 s.auth.privacy.changePolicy(s.auth.privacy.capture(),'block');await recovery;late.resolve(Response.json(ready()));await Promise.resolve()
 expect(s.thread.attachReadyMedia()).toBe(false);expect(s.media.read()).toMatchObject({phase:'cancelled',attachment:null,previewUrl:null});expect(s.thread.read().draft.media_ids).toEqual([])
})
```

</details>

<details>
<summary>consumer-guards.test.tsx</summary>

```tsx
// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { act } from 'react'
import { cleanup, render, screen } from '@testing-library/react'
import { FeedImage } from '../src/features/event-feed/FeedImage'
import { createSessionController } from '../src/features/auth/model/session'
const uid=(n:number)=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`
const callbacks:(()=>void)[]=[]
afterEach(()=>{cleanup();callbacks.splice(0).forEach(f=>f());vi.unstubAllGlobals()})
it.each(['protected-api','remote','cross-user','cross-event','stale-generation','wrong-id','wrong-purpose','wrong-path'] as const)('consumer refuses unqualified img source/state: %s',async mode=>{
 vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT',true)
 const auth=createSessionController({eventId:uid(2)});auth.session.replace(0,{userId:uid(1),eventId:uid(2),accessToken:'fixture-only'});callbacks.push(()=>auth.dispose())
 const scope={...auth.session.read(),...(mode==='cross-user'?{userId:uid(99)}:mode==='cross-event'?{eventId:uid(99)}:mode==='stale-generation'?{generation:-1}:{})}
 const path=`/api/concert/events/${uid(2)}/media/${uid(3)}/content`
 const state={phase:'ready' as const,scope,previewUrl:mode==='protected-api'?path:mode==='remote'?'https://unvalidated.invalid/image.png':'blob:fixture-only',error:null,
 media:{media_id:mode==='wrong-id'?uid(99):uid(3),purpose:mode==='wrong-purpose'?'REPORT' as const:'POST' as const,read_path:mode==='wrong-path'?'https://unvalidated.invalid/image.png':path,width:1,height:1}}
 const createPreview=()=>({read:()=>state,subscribe:()=>()=>{},load:async()=>{},cancel:()=>{},dispose:()=>{}})
 await act(async()=>{render(<FeedImage createPreview={createPreview} privacy={auth.privacy} mediaId={uid(3)} eventId={uid(2)} alt="Private image" />)})
 expect(screen.queryByRole('img')).toBeNull();expect(document.querySelector('[src]')).toBeNull()
})
```

</details>

<details>
<summary>browser.tsx</summary>

```tsx
import { StrictMode, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { createSessionController } from '../src/features/auth/model/session'
import { createImageUploadController, createMediaUploadTransport } from '../src/features/media/model/upload-controller'
import { createMediaPreviewController } from '../src/shared/media/read-media'
import { createFeedThreadController } from '../src/features/feed-thread/controller'
import { PostComposer } from '../src/features/feed-thread/FeedThread'
import { FeedImage } from '../src/features/event-feed/FeedImage'
const uid=(n:number)=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`
const eventId=uid(2),userId=uid(1),mediaId=uid(3)
const auth=createSessionController({eventId});auth.session.replace(0,{userId,eventId,accessToken:'qa-fixture-only'})
const media=createImageUploadController({session:auth.session,transport:createMediaUploadTransport(auth.request),purpose:'POST'})
const thread=createFeedThreadController({auth,media,access:{read:()=>({available:true,readOnly:false}),subscribe:()=>()=>{}}});thread.start()
const readers:ReturnType<typeof createMediaPreviewController>[]=[]
const createPreview=(id:string)=>{const controller=createMediaPreviewController({session:auth.session,request:auth.request,mediaId:id,purpose:'POST'});readers.push(controller);return controller}
const qa={auth,media,thread,readers,showImage:()=>{},hideImage:()=>{},invalidate:(kind:string)=>{
 if(kind==='cancel'){media.cancel();readers.forEach(r=>r.cancel())}
 else if(kind==='policy')auth.privacy.changePolicy(auth.privacy.capture(),'block')
 else if(kind==='unmount')qa.hideImage()
 else auth.session.replace(auth.session.read().generation,kind==='logout'?null:{userId:kind==='account'?uid(91):userId,eventId:kind==='event'?uid(92):eventId,accessToken:'qa-next-fixture-only'})
},cleanup:()=>{thread.dispose();media.dispose();readers.forEach(r=>r.dispose());auth.dispose()}}
;(window as any).qa=qa
function Fixture(){const [visible,show]=useState(false);qa.showImage=()=>show(true);qa.hideImage=()=>show(false)
 return <main><h1>TEST ONLY — independent privacy probe</h1><PostComposer controller={thread} onPost={()=>{}} />
 {visible&&<FeedImage createPreview={createPreview} privacy={auth.privacy} mediaId={mediaId} eventId={eventId} alt="Private fixture image" />}</main>}
createRoot(document.getElementById('root')!).render(<StrictMode><Fixture /></StrictMode>)
```

</details>

<details>
<summary>browser.html</summary>

```html
<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Independent media privacy QA fixture</title></head><body><div id="root"></div><script type="module" src="/.qa-media/browser.tsx"></script></body></html>
```

</details>

<details>
<summary>browser.mjs</summary>

```js
import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { writeFile } from 'node:fs/promises'
const uid=n=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`
const event=uid(2),mediaId=uid(3)
let stage='ready',pending=[],api=[],png,compressedType='image/png',uploadBytes
const ready=()=>({media_id:mediaId,state:'READY',revision:3,width:64,height:64,content_type:compressedType,purpose:'POST',read_path:`/api/concert/events/${event}/media/${mediaId}/content`})
const dev=await createServer({plugins:[{name:'independent-media-privacy-api',configureServer(server){server.middlewares.use(async(req,res,next)=>{
 if(!req.url?.startsWith('/api/concert/'))return next()
 assert.equal(req.headers.authorization,'Bearer qa-fixture-only');api.push({method:req.method,path:req.url})
 res.setHeader('Cache-Control','no-store')
 const path=new URL(req.url,`http://${req.headers.host}`).pathname
 const json=value=>{res.setHeader('Content-Type','application/json');res.end(JSON.stringify(value))}
 const hold=fn=>pending.push({res,send:()=>{if(!res.destroyed)fn()}})
 if(req.method==='GET'){
  if(path.endsWith('/content')){res.setHeader('Content-Type',compressedType);if(stage==='binary'){res.write(png.subarray(0,4));return hold(()=>res.end(png.subarray(4)))}return res.end(png)}
  if(stage==='metadata')return hold(()=>json(ready()))
  return json(ready())
 }
 assert.equal(req.headers['x-mcr-intent'],'browser');assert.match(req.headers['idempotency-key'],/^[0-9a-f-]{36}$/)
 const chunks=[];for await(const chunk of req)chunks.push(chunk);const body=Buffer.concat(chunks)
 if(path.endsWith('/uploads')){compressedType=JSON.parse(body).content_type;return json({media_id:mediaId,state:'PENDING',upload_path:`/api/concert/events/${event}/media/${mediaId}/bytes`,max_bytes:10485760,expires_at:'2099-01-01T00:00:00Z',upload_method:'PUT',revision:1})}
 if(path.endsWith('/bytes')){uploadBytes=body;return json({media_id:mediaId,state:'UPLOADED',revision:2})}
 if(path.endsWith('/finalize'))return stage==='finalize'?hold(()=>json(ready())):json(ready())
 res.statusCode=404;json({})
})}}],server:{host:'127.0.0.1',port:0,strictPort:false}})
let browser
const results=[],errors=[]
async function pageFixture(){const page=await browser.newPage({viewport:{width:390,height:844}})
 page.on('pageerror',e=>errors.push(e.message))
 await page.addInitScript(()=>{
  const metrics={created:[],revoked:[],activeWorkers:0,peakWorkers:0,aborted:0,fetches:0};window.metrics=metrics
  const create=URL.createObjectURL.bind(URL),revoke=URL.revokeObjectURL.bind(URL)
  URL.createObjectURL=blob=>{const url=create(blob);metrics.created.push(url);return url};URL.revokeObjectURL=url=>{metrics.revoked.push(url);revoke(url)}
  const NativeWorker=window.Worker;window.Worker=class extends NativeWorker{constructor(...args){super(...args);this.ownedActive=true;metrics.activeWorkers++;metrics.peakWorkers=Math.max(metrics.peakWorkers,metrics.activeWorkers);if(window.abortWorkerOnStart)queueMicrotask(()=>window.qa.invalidate('policy'))}terminate(){if(this.ownedActive){this.ownedActive=false;metrics.activeWorkers--}return super.terminate()}}
  const fetch=window.fetch.bind(window);window.fetch=(input,options)=>{const signal=input instanceof Request?input.signal:options?.signal;const url=input instanceof Request?input.url:String(input);if(url.includes('/api/concert/')){metrics.fetches++;signal?.addEventListener('abort',()=>metrics.aborted++,{once:true})}return fetch(input,options)}
 })
 await page.goto(`http://127.0.0.1:${dev.httpServer.address().port}/.qa-media/browser.html`);await page.waitForFunction(()=>!!window.qa)
 return page
}
try{
 await dev.listen();browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
 const seed=await pageFixture();png=Buffer.from(await seed.evaluate(()=>{const canvas=document.createElement('canvas');canvas.width=canvas.height=64;const c=canvas.getContext('2d');c.fillStyle='#51485c';c.fillRect(0,0,64,64);return canvas.toDataURL('image/png').split(',')[1]}),'base64');await seed.close()
 if(!process.env.QA_ONLY_WORKER){
 for(const held of ['metadata','binary'])for(const kind of ['cancel','logout','account','event','policy','unmount']){
  stage=held;pending=[];api=[];compressedType='image/png'
  const page=await pageFixture();await page.evaluate(()=>window.qa.showImage());await expect.poll(()=>pending.filter(x=>!x.res.destroyed).length).toBeGreaterThan(0)
  await page.evaluate(kind=>window.qa.invalidate(kind),kind)
  await expect.poll(()=>page.evaluate(()=>window.qa.readers.every(r=>r.read().phase==='cancelled'))).toBe(true)
  pending.forEach(p=>p.send());await expect(page.locator('img')).toHaveCount(0)
  await expect.poll(()=>page.evaluate(()=>window.metrics.aborted)).toBeGreaterThan(0)
  const metrics=await page.evaluate(()=>window.metrics);assert.deepEqual(metrics.created,[]);assert.equal(metrics.activeWorkers,0)
  results.push({case:`late-${held}-${kind}`,result:'PASS',abortedRequests:metrics.aborted,createdURLs:metrics.created.length,api:api.length});await page.evaluate(()=>window.qa.cleanup());await page.close()
 }
 // Native ready image owns a blob URL, revoked and removed at policy cleanup.
 stage='ready';pending=[];compressedType='image/png'
 const visible=await pageFixture();await visible.evaluate(()=>window.qa.showImage());const image=visible.getByRole('img',{name:'Private fixture image'});await expect(image).toHaveJSProperty('naturalWidth',64)
 const src=await image.getAttribute('src');assert(src.startsWith('blob:'));assert(!src.includes('/api/'));await visible.evaluate(()=>window.qa.invalidate('policy'));await expect(image).toHaveCount(0)
 assert.equal(await visible.evaluate(async url=>{try{await fetch(url);return false}catch{return true}},src),true)
 const shown=await visible.evaluate(()=>window.metrics);assert(shown.revoked.includes(src));results.push({case:'visible-policy-revokes-real-blob',result:'PASS',created:shown.created.length,revoked:shown.revoked.length});await visible.evaluate(()=>window.qa.cleanup());await visible.close()
 // Actual bundled worker prepares an upload; late finalize cannot reattach after a policy bump.
 stage='finalize';pending=[];api=[]
 const uploading=await pageFixture();await uploading.locator('input[type=file]').setInputFiles({name:'qa-private.png',mimeType:'image/png',buffer:png})
 await expect(uploading.getByRole('button',{name:'Upload selected image'})).toBeVisible();await uploading.getByRole('button',{name:'Upload selected image'}).click()
 await expect.poll(()=>pending.filter(x=>!x.res.destroyed).length).toBeGreaterThan(0)
 const before=await uploading.evaluate(()=>({phase:window.qa.media.read().phase,url:window.qa.media.read().previewUrl,metrics:window.metrics}));assert.equal(before.phase,'finalizing');assert(before.url.startsWith('blob:'));assert(before.metrics.peakWorkers>=1)
 await uploading.evaluate(()=>window.qa.invalidate('policy'));pending.forEach(p=>p.send())
 await expect.poll(()=>uploading.evaluate(()=>window.qa.media.read().phase)).toBe('cancelled');assert.equal(await uploading.evaluate(()=>window.qa.thread.attachReadyMedia()),false)
 const after=await uploading.evaluate(()=>({state:window.qa.media.read(),ids:window.qa.thread.read().draft.media_ids,metrics:window.metrics}));assert.equal(after.state.attachment,null);assert.deepEqual(after.ids,[]);assert.equal(after.metrics.activeWorkers,0);assert(after.metrics.revoked.includes(before.url))
 assert.equal(await uploading.evaluate(async url=>{try{await fetch(url);return false}catch{return true}},before.url),true)
 results.push({case:'late-finalize-policy-real-worker',result:'PASS',peakWorkers:after.metrics.peakWorkers,liveWorkers:0,attachment:null,compressedType,uploadBytes:uploadBytes.length});await uploading.evaluate(()=>window.qa.cleanup());await uploading.close()
 }
 stage='ready';pending=[];api=[]
 const aborting=await pageFixture();await aborting.evaluate(()=>window.abortWorkerOnStart=true);await aborting.locator('input[type=file]').setInputFiles({name:'qa-aborted-worker.png',mimeType:'image/png',buffer:png})
 await expect.poll(()=>aborting.evaluate(()=>window.metrics.peakWorkers)).toBe(1);await expect.poll(()=>aborting.evaluate(()=>window.qa.media.read().phase)).toBe('cancelled')
 const cancelled=await aborting.evaluate(()=>({state:window.qa.media.read(),ids:window.qa.thread.read().draft.media_ids,metrics:window.metrics}));assert.equal(cancelled.metrics.activeWorkers,0);assert.deepEqual(cancelled.metrics.created,[]);assert.equal(cancelled.state.previewUrl,null);assert.equal(cancelled.state.attachment,null);assert.deepEqual(cancelled.ids,[]);assert.equal(api.length,0)
 results.push({case:'policy-cancels-actual-worker-before-result',result:'PASS',peakWorkers:1,liveWorkers:0,createdURLs:0,apiRequests:0});await aborting.evaluate(()=>window.qa.cleanup());await aborting.close()
 assert.deepEqual(errors,[]);const report={result:'PASS',candidate:'5c95ed4',scope:'Independent actual FE04/FE28 controllers/canonical auth+HTTP/module worker/Blob DOM; native deterministic backend only',browser:await browser.version(),results,errors}
 await writeFile(process.env.QA_ONLY_WORKER?'.qa-media/worker-browser-results.json':'.qa-media/browser-results.json',JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report))
}finally{await browser?.close();await dev.close()}
```

</details>
