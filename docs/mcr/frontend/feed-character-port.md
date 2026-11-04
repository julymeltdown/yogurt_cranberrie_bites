# FE-FEED-CHARACTER-PORT: existing author renderer qualification

Status: CONTRACT_READY for the existing public callback, with fixture-native resource/identity/interaction evidence. This handoff adds only `frontend/src/features/event-feed/__tests__/character-port/**` and this document. Production EventFeed/index, controllers/models/contracts, CSS, root, availability, art, dependencies and lockfiles are unchanged. No new callback or props interface is needed.

## Existing supported interface

The production implementation and public export already exist in source commit `10978fab7b341afd2b3c36cdbe2e448af249c69a`. Read them directly in `frontend/src/features/event-feed/EventFeed.tsx` and `index.ts`:

```tsx
import { EventFeed, type RenderPostAuthor } from 'features/event-feed'
// Existing public type:
// type RenderPostAuthor = (author: Post['author']) => ReactNode
```

`EventFeed` accepts optional `renderAuthor`. It forwards that callback to `PostContent`, which supplies the exact currently loaded, schema-decoded and frozen `Post.author`: `user_id`, `display_name`, `bio`, and canonical `avatar: {revision,config}`. Config is the actual 1.0 layered / 1.1 whole-character union. The callback is a render function and may run again on ordinary React renders. It must return presentation and remain free of imperative network/runtime creation or hook calls. A root component obtains its already-owned current runtime outside the callback and returns a borrowed character view from the callback.

The output occupies only the existing 48×48 `.feed-portrait` slot. Feed still renders the author name, created/edited time, text, media and actions. Do not duplicate names/links/headings inside the portrait. `renderAuthor` does not navigate, read media, establish authorization, mutate state, or qualify Feed availability. `onPost`, `onCompose`, `onLink`, and `renderMedia` retain their existing separate meanings.

- With no callback (or `undefined`), the exact existing fallback remains a `.feed-initial` span, `role="img"`, label `${display_name}: avatar unavailable`, and the uppercase first Unicode code point of the trimmed name, or `?`.
- With a callback, its React output is used unchanged. `null` intentionally leaves the portrait slot empty; EventFeed does not add initials or select character art behind the caller's back. A consumer that needs an unavailable label must render it explicitly or omit the callback.
- A supplied whole-character view must use this author's actual `avatar.config` and display name, under the root's current canonical scope. It must never use the current viewer's selected preset for other authors, an art-index mapping, a default selected character, or an unreviewed identity fallback.
- A 1.0 author remains 1.0. A whole-only consumer may return the same honest initial/unavailable presentation for that branch; a separately qualified layered renderer is also caller-owned. No cast or rewriting to 1.1 is allowed.
- Runtime identity belongs to the existing root registry. Components borrow it, acquire/release their own render leases, and never dispose the root runtime. No extra per-row runtime, connection or image owner is introduced by this seam.

Media/root implementation owners resolved the interface directly and may bind the existing callback independently of this handoff. There is no publication prerequisite or approval gate here. Root files remain under Media's exclusive ownership; `useCharacterRuntime` and root scope/lifetime plumbing remain their owners' implementation concerns.

## Evidence and source pins

Assignment checkout: retained isolated `fe-ref-gifts`, new branch `codex/mcr-fe-feed-character-port`, actual start base `08d1b1c048a92db3f55ce53c306694f2549df177`. Qualification reconciled master `903773b82836f6b76f2a2ece5b37ec686633a655`, preserving peer Media/Profile/Party/Landing/backend work. `evidence/qualification.json` records exact source hashes and final checks; `evidence/report.json` records browser, source hashes, geometry, pixels, resource reads, screenshot hashes and cleanup.

The new fixture pairs six explicitly labeled synthetic author IDs with the six actual frozen canonical 1.1 configs. It validates them using the real generated FeedPage schema through the actual Feed controller. These are declared test relationships, not real-user identity or inferred author-to-art mappings. Art and the real shared `WholeCharacterRuntime`/`WholeCharacterView` use the published reviewed descriptor/manifest/hash validation; supplied original/derived art bytes are unchanged.

Commands from repository root with Node 24.22.2 and existing pinned dependencies:

| Command | Result |
| --- | --- |
| `node frontend/node_modules/vitest/vitest.mjs run --root frontend src/features/event-feed/__tests__` | exit 0; 34 tests / 4 files, including 12 new port cases |
| `node frontend/node_modules/typescript/bin/tsc -p frontend/tsconfig.json --pretty false --incremental false` | exit 0 |
| `node frontend/node_modules/oxlint/bin/oxlint frontend/src/features/event-feed/__tests__/character-port` | exit 0; no warnings |
| `node frontend/src/features/event-feed/__tests__/character-port/native.mjs` | exit 0; native Chrome 156.0.8078.80 at 360/390/430, three full-page plus eighteen centered portrait captures |

Unit cases cover pending/denied/mixed-invalid data before callback invocation, all six admitted author identities/revisions, frozen unchanged values, exact no-callback markup, null output, retained 1.0 union input, explicit refresh to a changed author config, callback replacement, access withdrawal, suspension, identity logout, read-only presentation, preserved thread/link/compose callbacks and GET-only fixture requests.

Native evidence exercises the actual production EventFeed/public callback/parser/canonical privacy together with genuine selected art and the existing production portrait CSS. All six backing-canvas RGBA hashes are distinct, each direct 64px character view fits inside its existing 48px slot without clipping, and smoothing is disabled. One descriptor plus one compact atlas serves every author; no detail atlas or per-author asset request is made. NOTICE navigation, callback omission/reinstallation and Feed unmount/remount release only the consumer leases, preserve the current owner, and reuse its verified atlas. Reloading the first author's config draws the sixth preset's exact existing pixel hash. Read-only keeps portraits and hides compose. Closed-gate suspension clears pixels synchronously before deferred React work, removes all portrait DOM, disposes images/leases and reports zero cleanup failures. Thread keyboard entry, related-link and compose callbacks remain independent, with no business writes. Fallback/null/denied cases perform no art reads. No page errors or horizontal overflow occurred.

The full-page fixture screenshot uses the retained production sticky footer; centered individual portrait captures ensure that footer does not conceal identity evidence. [390px Feed fixture](../../../frontend/src/features/event-feed/__tests__/character-port/evidence/loaded-390.png), [fifth author slot](../../../frontend/src/features/event-feed/__tests__/character-port/evidence/author-5-390.png), [native report](../../../frontend/src/features/event-feed/__tests__/character-port/evidence/report.json).

During harness development, an initial probe wrongly treated `changePolicy('block')` as a closed gate. Actual `shared/query/privacy.ts` increments the policy epoch for block while retaining an open gate; current-scope reads may resume. `suspended` is the correct closed-gate probe. The corrected tests use suspension for persistent DOM eviction. This was a fixture expectation error, not evidence of a Feed authorization defect. Real root availability is additionally owned by its original canonical scope and availability lease. No model guard or authority change was made.

## Delivery boundary

This qualifies the existing producer callback and its borrowed-renderer behavior. Media owns separate actual route binding and its validation; this fixture does not certify that binding, public/authenticated Feed backend readiness, real content access, real users, physical devices, multiuser/release acceptance, or a live business mutation. Live Feed production availability remains false until its actual qualified producers establish readiness. A callback, fixture result or supplied portrait does not enable writes/uploads or change readiness.

Implementation owners exclusively choose and perform code/query inspection, validation, fixes and direct non-force publication. Kings/SLs track delivery/dependencies/staffing and do not impose technical review or rerun gates. Next action belongs to the root/Media owner for its independent Feed binding; concrete scoped producer findings can return directly to this owner.

Publication reconciliation parent: `aa3e2f3d67afc1f288aee63f994dc829fcc81e7b`. All recorded producer/schema/privacy/runtime source hashes remained unchanged. Peer Media published separate route binding source `18418f63` within `9dacd96237760bd65d450434b3dd1e490ea0c914`; that work is preserved and was not re-qualified by this component fixture. No repeated native run was needed for those disjoint changes.
