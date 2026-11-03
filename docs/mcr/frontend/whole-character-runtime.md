# Selected whole-character runtime

Task: FE-STATIC-AVATAR-RUNTIME. Component status: INTEGRATED producer, native resource/render checks passed. Complete product acceptance remains with the root, CUSTOMIZE, World and persistence owners. This patch changes only `frontend/src/entities/avatar/whole-character/**` and this document.

## Provenance and supported identity

Assignment base: `e9ce93daf2c0d6aa453d055efad271d1de34349c`. Reconciled validation base: `f4b4080897a6b1ad835fa4e5e9372f82d58e489e`, including compatibility repair `90f44fd99fdfe17f3ee3e596ceff0583f6db2c47` and the canonical root lifetime registry. Canonical contract source: `3398d8331863775ecc0180e5eccebddd179fedd5`. Reviewed art source: `7e07f7328f079991a9aedb7ad83a4b682decb543`. The implementation commit containing this document is the producer handoff; these source revisions are its ancestors.

The actual checked-in config, manifest and common JSON schemas are imported and their whole-character definitions evaluated with a bounded fail-closed evaluator. This is a consumer of the canonical schema, not a second wire contract. Unknown schema keywords fail validation. Whole-character input is the exact six-key AvatarConfig 1.1.0 with representation `whole_character`, manifest `mcr_selected_static_20261103` version 1, and one of `photo01_front_01` through `photo01_front_06`. Identity never comes from an art index, thumbnail URL or default preset. Configs and manifests returned by validation are frozen copies.

The only capability is `front_idle`. Six logical 64×64 cells have foot anchor (32,60). No layers, skin tones, facing directions, walking frames, palette mutation or legacy art substitution are supplied. Art IDs retain hyphens (`photo01-front-01`); public preset IDs retain underscores (`photo01_front_01`).

| Reviewed input | SHA-256 | Bytes |
| --- | --- | ---: |
| Manifest canonical content | `0627c7d178b22535a561e1270db6d8bf9e2383ba84d03a63962e48e565c59b2b` | bounded to 32768 |
| `world/avatars/derived-v1/static-release.json` | `cc53c6852742e9cbb9858a2d0961411e1a9e131d1e30c0f2b73b265e03f91677` | 1898 |
| `photo01-front-idle-64-e124c88815b22220.png` | `e124c88815b22220df2f2dc59e2f26b3ceb845e57b77674fd8e55383116d7efe` | 36030 |
| `photo01-front-idle-256-6258b9f9ee51aed7.png` | `6258b9f9ee51aed7a04b0690dfb6fc20592ed8347d72fcaaf8037b4c19e7b099` | 368812 |

Source photo hash: `67448360482a0dd68172d6089378a002190142bb8bd804818c89a86370154311`. Originals and derivatives remain untouched. No code is copied from historical rendering: the producer is newly authored and imports existing audited headless JSON/image helpers, the selected-static release validator, React, and canonical auth/privacy types. It adds no dependency or lockfile change.

## Consumer API and ownership

Import from `entities/avatar/whole-character`. `createWholeCharacterRuntime({privacy,manifest,pin,origin,fetch?,decode?,timeoutMs?})` captures one full current canonical PrivacyScope. Root creates one runtime for that scope, shares it with Header/Me/CUSTOMIZE/World and owns disposal. Runtime never rebinds to a new user/event. Canonical privacy cleanup also disposes it immediately. The root registry must compare canonical scope before and after constructing the runtime; an absent identity or closed gate is rejected.

- `choices(signal)` verifies descriptor and manifest, returns six immutable `{id,label,config}` choices and fetches no PNG.
- `validateConfig(input)` rejects invalid or stale-owner input. Pure `validateWholeCharacterConfig`, `validateWholeCharacterManifest`, and `sameWholeCharacterConfig` are also exported.
- `acquire(config,{signal,detail?})` returns a borrowed render lease. Default requests only the 384×64 atlas; explicit detail requests only the 1536×256 atlas. Both crop the same identity and retain logical anchor (32,60).
- Lease `read()` returns readonly `{config,pose,image,source,anchor,logicalCell}` or null immediately on cancellation/revocation. `subscribe(callback)` clears consumer sinks synchronously; one failed subscriber does not prevent others. `dispose()` releases this consumer only. Consumers must not mutate the shared image and must dispose their own GPU textures on notification.
- `WholeCharacterView({runtime,config,label,size?,detail?,className?})` renders a pixelated canvas at 64/128/256, owns its abort signal and lease, and clears pixels on revoke/unmount/config replacement. It never disposes the shared runtime. Invalid/unavailable input renders an accessible unavailable state.

Per owner there is at most one metadata flight, one compact atlas and one detail atlas, with no global cache or shared identity. At most 256 pending/active render leases are admitted. Independent caller cancellation preserves other waiters; the last pending waiter aborts its flight. Requests and decodes are deadline bounded (1–15000ms); abandoned late decodes are disposed. Every fetch checks exact same-origin URL, redirect refusal, MIME, exact byte count/stream ceiling, SHA-256, PNG header and decoded dimensions. Requests omit credentials and referrer. One cleanup failure does not block other resource cleanup.

## Validation and evidence

All commands below exited 0 on the reconciled base unless explicitly labeled otherwise. Node 24.22.2 and existing pinned frontend dependencies were used. Browser/build work ran above the 8GiB free-disk guard. Native fixture and artifacts live in `frontend/src/entities/avatar/whole-character/__tests__/native/`; its `evidence/report.json` records browser version, screenshot hashes, pixel hashes and zero-resource final states.

| Check | Command from repository root | Result |
| --- | --- | --- |
| Producer + compatible integration boundaries | `node frontend/node_modules/vitest/vitest.mjs run --root frontend src/entities/avatar/whole-character/__tests__/contract.test.ts src/entities/avatar/whole-character/__tests__/runtime.test.ts src/entities/avatar/whole-admission/__tests__/compiler-compatibility.test.ts src/app/character-entry/__tests__/registry.test.ts` | 36 tests / 4 files; 26 producer tests |
| Types | `node frontend/node_modules/typescript/bin/tsc -p frontend/tsconfig.json --pretty false --incremental false` | PASS |
| Owned source lint | `node frontend/node_modules/oxlint/bin/oxlint frontend/src/entities/avatar/whole-character --ignore-pattern '**/build/**'` | PASS |
| App production bundling | `node frontend/node_modules/vite/bin/vite.js build frontend --outDir src/entities/avatar/whole-character/__tests__/native/build` | PASS; existing large Phaser chunk warning |
| Actual React/runtime native proof | `node frontend/src/entities/avatar/whole-character/__tests__/native/run.mjs` | PASS; 9 captures at 360/390/430, 6 distinct pixel identities, stable across 3 era backdrops |
| Canonical asset metadata | `/tmp/mcr-fe14-format-venv/bin/python scripts/mcr/validate-assets.py --fixtures contracts/fixtures/assets/selected-static` | PASS; 7 golden documents |
| Planning provenance | `python3 scripts/mcr/check-plan.py` | PASS |

Producer adversarial tests cover invalid/mixed identity, lexical duplicate JSON keys, pin/source/frame binding, capacity, metadata-only choice discovery, explicit detail, shared compact loading, independent cancellation, canonical policy/actor change, wrong hash/MIME/redirect/length/stream size, ignored late decoders, deadlines and throwing cleanup/subscribers. Native proof uses real canonical privacy machinery with fixture session identity and actual browser SHA/decode/canvas behavior. It verifies keyboard preset selection, unchanged pixels across backdrop switches, compact atlas reuse, no detail request until explicit action, unmount preserving the owner, remount without refetch, immediate same-evaluation pixel clearing after canonical revoke, zero retained leases/images and no page errors. Representative 360/390/430 screenshots were visually inspected for six readable characters, preserved light clothing and no overflow.

The default system Python asset check initially exited 1 because URI/date-time checker dependencies were absent; the existing contracts venv passed the same check. Supplementary `python3 -m unittest scripts/mcr/tests/test_selected_static.py` could not load because this frontend sparse checkout excludes `backend/scripts/docs/generate-mcr-core.py`; it is NOT_RUN here, not a passing full contract suite. No backend path was changed to bypass that prerequisite.

## Remaining integration boundaries

These results certify this bounded producer and native fixture. They do not certify live login/bootstrap delivery, save/reload persistence, remote-player identity admission, Phaser/world rendering, transport, physical device behavior or the complete product journey. Admission union decoding, root composition/lifetime plumbing, feature editor state, old layered behavior and World single-frame texture disposal are separately owned. No root file, legacy avatar entrypoint, contract, art manifest, original, shared transport, feature implementation or dependency was modified by this patch.

Publication reconciliation: replayed the single producer commit onto `be629fa0951f047710751d0137c1ec6a6f34a6ea`, preserving concurrent legacy asset leases and coordination updates. The same 36 targeted tests and full TypeScript check passed again (exit 0); producer paths remained disjoint.
