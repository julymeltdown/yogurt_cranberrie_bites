# C03 draft contract review — C06 handoff

Review status: **four bounded findings remain in the inspected draft**. This is a read-only contract review, not an implementation, runtime privacy audit, C03 acceptance, or OAuth qualification. Only this review document is authored by this follow-up.

Reviewed main checkout: `/Users/lhs/Desktop/mcr`. The reviewed draft was concurrently being authored; the last observed main HEAD was `82a5b76004c55cb3ca01d8706604156f2493bbb7`, and the C03 inputs were uncommitted. The content hashes below identify the actual inspected inputs rather than implying they belong to that HEAD. Review branch: `codex/mcr-contracts-C06`, starting at `efeedf684bfc7c48e76a933dc68b4cfc2075cc6f`. The review adds no changes to C06 schemas or other files.

The governing requirements are specification §§5.2–5.4 (shared theme identity and physical/virtual separation), 7.1 (one contract source), 7.2 (Auth identity and session handling), 7.4 (Bootstrap, public/private data and typed error reasons), and C03/BE06–BE08. C06 v1.0.0 supplies the exact imported asset definitions.

## Findings

### C03-R01 — P2: core error fixtures cannot enforce the reason contract or detect unintended private details

Location: `backend/scripts/docs/generate-mcr-core.py:148`–153; `backend/scripts/docs/openapi/schemas.py:14`–24; `scripts/mcr/validate-core.py:47`–76. The generator uses inherited `ErrorResponse` for every new core error and only changes its `traceId` schema. The inherited nullable/open `details` object has no `reason` constraint, and the new semantic validator has no error-specific check.

Reproduction with the new validator: load `error-version-conflict.json`, replace `details` with each of `null`, `{"reason": []}`, and `{"reason": "UNREGISTERED_REASON", "private_handle": "sensitive"}`, then call `validate(value, "ErrorResponse", document)`. **All three pass**. This is a contract-test gap, not evidence that a real service currently leaks a handle. The existing nine golden error examples validate because the examples are well-formed; their schemas do not enforce the machine-readable behavior described in the contract document.

Impact: FE cannot rely on the checked error contract to distinguish `VERSION_CONFLICT`, `MAP_VERSION_MISMATCH`, `RESOURCE_UNAVAILABLE`, or other specified reasons, and fixture validation can bless unintended sensitive error metadata. Specification §7.4 requires machine decisions to use `details.reason` and forbids detailed private policy reasons in public responses.

Bounded fix: preserve the inherited error envelope/camelCase but add an MCR-specific error definition or operation-aware semantic rule for the new core operations. Require an allowed string reason for errors that promise one; define allowed safe details per reason rather than allowing arbitrary private properties. Keep generic inherited auth errors separate where a reason is legitimately absent. Add negative cases for missing/non-string/unknown reasons and private detail keys, including the actual response-to-operation/status mapping. Do not require modifying the inherited runtime contract to fix the MCR target contract.

### C03-R02 — P2: place response validation ignores the supplied requested event

Location: `scripts/mcr/validate-core.py:47`–76; `contracts/core/v1/core.schema.json:426`–473; `contracts/fixtures/core/index.json` entries for `place.json` and `place-list.json`.

Reproduction: load the supplied `place.json` or `place-list.json` (payload event UUID ends in `0201`), then call `validate(value, schema_name, document, event_id="00000000-0000-4000-8000-000000000999")`. **Both pass**. The optional event context is enforced only for `PrivateParticipant` and transitively for `Bootstrap`. Place fixtures do not record requested event context. `PlaceResponse` also has no expected-place context: changing its nested `place_id` to another valid UUID still passes.

Impact: the place producer/consumer fixture gate can certify an event-A response for an event-B URL, or a different place for a requested place URL. Current closure/provenance could be associated with the wrong event/place despite valid C06 field shapes. This does not establish a runtime authorization failure; no runtime is exercised.

Bounded fix: apply `event_id` binding to `PlaceList` and `PlaceResponse`; add explicit requested `place_id` context for the detail operation and record these values in the fixture index. Add cross-event and wrong-place negative tests. For `getConcertPublicProfile`, separately define target-user context for `{userId}`; the authenticated caller's `actor_id` must not be incorrectly equated with the target profile's user ID. JSON Schema cannot express arbitrary cross-document equality here; a small semantic validator is appropriate.

### C03-R03 — P2: the real theme-manifest delivery channel is unspecified

Location: `contracts/core/v1/core.schema.json:335`–373; `contracts/core/v1/operations.json:1`–66; `docs/mcr/core-contract.md:65`–90; C06 `ThemeManifest`.

The bundled OpenAPI contains the correct C06 `ThemeManifest` definition, but no core operation returns it, and Bootstrap contains only `map_manifest` and `avatar_manifest`. Preferences select `ThemeId`; neither the map nor avatar manifest supplies a theme URL/catalog. The reviewed document also supplies no immutable static manifest URL/version convention. A definition's presence in OpenAPI components is not a delivery operation. An in-memory query of generated paths found no theme/manifest operation, and Bootstrap has no theme-delivery field.

Impact: a real FE06/FE14 loader cannot discover the selected skin's texture/palette/revision/hash and verify its map/avatar bindings from the defined handoff without inventing another convention or relying on test fixtures. Spec §5.2 requires all three themes to preserve world identity, and C06 already specifies the manifest identity checks.

Bounded fix/decision: the parent should choose and document one delivery path before real asset consumers implement it: include versioned theme manifests or typed references in Bootstrap, expose a manifest-read operation, or specify an immutable static catalog/addressing contract. Reuse C06's exact definition. Add one consumer fixture/probe that resolves every ThemeId and verifies map/avatar content hashes, collision ID, place IDs and interaction IDs. This is an unresolved C03/C06 delivery decision; it does not require artwork, another asset engine, or changing C06's shared geometry model.

### C03-R04 — P2: public provider discovery is assigned authenticated refresh retry behavior

Location: `backend/scripts/docs/generate-mcr-core.py:156`–159; `contracts/core/v1/operations.json:3`–8; `docs/mcr/core-contract.md:58`–63.

The generator assigns `READ_ONCE_AFTER_REFRESH` to every new GET solely by HTTP method. Generated `GET /api/auth/capabilities` has `security: []` **and** `x-mcr-retry: READ_ONCE_AFTER_REFRESH`. The consumer decisions authorize this retry only for an authenticated read in the same session generation.

Impact: a client honoring the operation's retry annotation can start credential rotation in response to a failure of public provider discovery, before an authenticated session is established. The operation metadata contradicts the documented single-flight/session boundary. No browser refresh flow was run here.

Bounded fix: condition the annotation on both GET and `authenticated: true`; mark public capabilities with an explicit public-read/no-auth-refresh policy. Add a generated-operation assertion covering this distinction. Retain the existing no-automatic-replay policy for auth mutations and disabled OAuth handoffs.

## Confirmed compatibility and covered concerns

The C03 assembler builds 24 operations in memory. Every bundled local `$ref` resolves, including the exact `AvatarConfig`, `AvatarManifest`, `MapManifest`, `ThemeId`, `PlaceId`, and `Place` definitions supplied by C06. There is no duplicated asset DTO source in the inspected core schema. New DTOs use snake_case, inherited Auth fields retain camelCase, and the refresh credential is constrained to an empty public `refreshToken`.

The parent authored its fixtures/semantic validator during this review. The initial schema-only observations about duplicate providers, cross-event Bootstrap identity, private/profile identity, divergent Bootstrap avatar projections, unknown catalog items and read-only mutations are **covered by the arrived semantic validator and negative tests**, so they are not listed as outstanding findings. PublicProfile's top-level allowlist rejects the tested private fields. UTF-8 password byte validation is implemented as a custom validator keyword rather than falsely relying on JSON Schema character length.

OAuth operations are marked `DISABLED_UNTIL_HANDOFF_QUALIFIED`. The document explicitly records unresolved JSON/SPA callback handling, Apple's completion path, return targets and account-link binding. This review does not request OAuth enablement or treat these deliberate disabled states as a new defect. Likewise, target route metadata does not claim existing route registration, and metadata fixtures do not claim actual pixel/mobile/AR acceptance.

## Read-only evidence and snapshot

Commands executed with `PYTHONDONTWRITEBYTECODE=1` to prevent import-cache writes in main:

- `python3 -m unittest discover -s scripts/mcr/tests -p test_core.py -v`, cwd main: **exit 0, 15 tests pass**. The suite validates all 23 indexed golden fixtures, local references, deterministic/current generation, and the negative cases it currently covers.
- In-memory imports of `generate-mcr-core.py.build_document()` and `validate-core.py.validate()`: **exit 0**. No generator output-write entrypoint was invoked. Assembly/local-reference probes succeed; counterexample probes report the accepted invalid cases under R01/R02 and inspect the conflicting metadata under R04.
- Read-only path/component inspection establishes R03. No application, database, browser, device or load build/check was run.

| Inspected input relative to main | SHA-256 |
|---|---|
| `contracts/core/v1/core.schema.json` | `5199eb651b70f0b10cf18a9341bc8b4ceb8f5d5ef38889ab8f8e4bf025fcd453` |
| `contracts/core/v1/operations.json` | `2f5b972dbccea09340e1519a1a2c522e3c42f63690fa26b9e58d721e9927ba62` |
| `backend/scripts/docs/generate-mcr-core.py` | `7b845c1b329a0cfa0fc7d8001a410d0330f6679e15a4472367e33c8c32e079c1` |
| `docs/mcr/core-contract.md` | `a053e93664018be479609e01ca818ff64ed2031d9356abf3ebf97e76554a869c` |
| `scripts/mcr/validate-core.py` | `5a6b293e77bf8118d2533a460640b9c0f38759c02fb0e5668ce6341ec5d19f2b` |
| `scripts/mcr/tests/test_core.py` | `a10570959a2c8884beea88bc1ee74f84eaecc265f4d75417d90307cb879df027` |
| `contracts/fixtures/core/index.json` | `862aca8983894f31978bab2881801a8402f2fdbf01e924bd8a3e21b8fe4188c4` |

These are findings against this content snapshot. The parent may resolve them in later edits; recheck the owning source before applying them to another candidate. The only delivery for this follow-up is `docs/mcr/core-contract-review.md`, committed locally on the isolated worker branch. Parent integration/publication and all formal acceptance decisions remain with the integrator.

## Follow-up: completed draft and negative-coverage check

The parent's completed draft was rechecked after upstream-schema reachability pruning and its `Current validation` section were added. Core schema, operations, validator, tests and fixture-index hashes remain identical to the snapshot above. The updated generator hash is `60b1b0e4f6786df5e073bf6873f3b0557f07a5d23a5040f1ac85c306eda2792d`; updated `docs/mcr/core-contract.md` hash is `b819c380ec59c12b500f345febadb5b937aaf34a605bd5faffd4bc2917f46234`. Reachability pruning preserves the C06 definitions and the required inherited Auth schemas; the four findings above remain applicable.

Re-executed in main with `PYTHONDONTWRITEBYTECODE=1`: the 15-method `test_core.py` suite returns **exit 0**, and `python3 scripts/mcr/validate-core.py` returns **exit 0**, reporting 23 indexed fixtures, `DRAFT_VALIDATED`, producer runtime `NOT_RUN` and freeze `PENDING`. No output-generating entrypoint, application build, or runtime route execution was invoked.

The negative coverage actually present in those tests is:

| Boundary | Negative behavior checked | Evidence limit |
|---|---|---|
| Public profile privacy | Reject email, exact age, age claims, social handles, preferences, meeting and access-token extra properties | Schema allowlist; not real HTTP/WS/cache privacy |
| Private identity | Reject wrong actor, wrong event and nested profile/participant user mismatch | Context checks for PrivateParticipant, also used by Bootstrap; not yet place-response binding under R02 |
| Bootstrap consistency | Reject participant/event mismatch and divergent avatar revision/projection | In-memory response relationship validation |
| Avatar catalog membership | Reject an unknown hair item even when both Bootstrap avatar copies agree | Real C06 manifest lookup used by the validator; not a server mutation test |
| Read-only lifecycle | Reject mutation capability enabled on the read-only Bootstrap fixture | Contract consistency; not server write authorization |
| Provider capabilities | Reject duplicate/missing provider identities and enabled flag with a non-null disabled reason | Does not prove provider configuration or browser handoff; OAuth stays disabled pending decision |
| Credential/resource boundaries | Reject arbitrary avatar asset URL field and non-empty public `refreshToken` | Does not inspect actual cookies, logs, responses or browser persistence |
| Password policy | Reject too-short and oversized UTF-8 byte sequences, including multibyte overflow | Custom keyword enforces bytes in this Python validator; generated consumer parity still needs qualification |
| Update requests | Reject missing/zero expected revision and an update containing only its revision | Does not exercise stale-version conflicts or concurrent persistence |
| Clock/place shape | Reject non-UTC Bootstrap timestamp and duplicate place IDs | Does not establish venue truth, source rights or requested-place identity |

Positive checks additionally validate all 23 indexed fixtures, exact inherited auth operation mapping, local schema references, unique operation IDs, deterministic/current generated output, and explicit unimplemented/adaptation-required target states. Deliberate unimplemented runtime routes and disabled OAuth handoffs are **not bugs in this review**. The remaining findings concern the defined contract, metadata or fixture gate itself.

## Resolution recheck: C03-R01 through C03-R04 closed for the inspected patch

**All four findings are resolved in the inspected pending main patch.** This resolution supersedes the earlier statements that four findings remain. The original findings and prior snapshots above are retained as review history. Main HEAD observed at the start of this recheck was `ea0c55771880dc483435f46bfb05a626e2c1b136`; the content hashes below identify the pending draft actually inspected.

| Finding | Resolution verified | Re-executed counterexample evidence |
|---|---|---|
| R01: core error reasons/private details | New strict `CoreErrorResponse` and per-reason closed `CoreErrorDetails` preserve the inherited envelope field names while constraining MCR errors. Generated core response schemas additionally restrict reasons by status; fixture context validates the actual operation/status response. Generic inherited Auth `ErrorResponse` remains separate. | Reject null/empty details, array reason, unknown reason, and a valid reason accompanied by `private_handle`. Reject VERSION_CONFLICT at the avatar operation's 404 response. Passing generic `ErrorResponse` as the initial schema cannot bypass the stricter core operation's 409 response. A genuine inherited login 401 error still validates. |
| R02: requested identity binding | `PlaceResponse`/`PlaceList` now enforce requested `event_id`; detail enforces requested `place_id`; `PublicProfile` supports separate `target_user_id`. The fixture index carries the relevant expected identities. | Both wrong-event place counterexamples reject, as do wrong requested place and wrong target user. A public profile remains valid when its target differs from the authenticated caller, avoiding accidental caller/target conflation. |
| R03: theme delivery and consistency | Bootstrap now requires exactly three `theme_manifests`, using the existing C06 definition. Core semantic validation delegates the full config/avatar/map/theme bundle to C06's validator. | Rehashed theme mismatches for map ID, map revision, collision ID, place IDs and avatar content hash all reject. Missing third theme and duplicate ThemeId reject. Rehashing before these checks confirms semantic binding rather than merely detecting a stale digest. |
| R04: public capability refresh policy | Public discovery has `security: []` and `PUBLIC_READ_NO_AUTH_REFRESH`. Authenticated Bootstrap reads retain `READ_ONCE_AFTER_REFRESH`. | Direct generated-operation assertions confirm both public and authenticated policies. No runtime credential rotation is inferred from this metadata check. |

The updated `scripts/mcr/tests/test_core.py` was read in full. Its five added methods cover public discovery retry separation; requested place/event/target-user identities; required reason and safe details; operation/status response validation; and three delivered, consistent theme manifests. Existing privacy, Bootstrap identity/avatar, read-only event, provider identity, revision, UTC, reference-resolution and deterministic-generation checks remain. Password tests now also reject invalid Unicode without escaping as an encoding exception.

Executed read-only in main with `PYTHONDONTWRITEBYTECODE=1`:

- `python3 -m unittest discover -s scripts/mcr/tests -p test_core.py -v`: **exit 0; 20 test methods pass**, including generated-artifact determinism/currentness.
- `python3 scripts/mcr/validate-core.py`: **exit 0; 25 indexed fixtures**, state `DRAFT_VALIDATED`, producer runtime `NOT_RUN`, contract freeze `PENDING`.
- Additional in-memory replays: **exit 0; all 18 negative counterexample probes reject**. Positive controls preserve generic Auth errors and caller/target separation. Generated target still has 24 operations. These probes write no executable, fixture, schema, or generated output.

| Rechecked input relative to main | SHA-256 |
|---|---|
| `contracts/core/v1/core.schema.json` | `f7f0ef7ce4554301a21775eab80df817420794ebb3494f0d18346f612070f079` |
| `backend/scripts/docs/generate-mcr-core.py` | `12d0ea30ccc3e7599eec5f9148db682d1c90eab84a6f4da8f1cd9d0431617cd5` |
| `scripts/mcr/validate-core.py` | `916508b720cc072a0a7a00069171b1ce10a826a53aaaea41f24b64397687b4b6` |
| `scripts/mcr/tests/test_core.py` | `5d7b1c2c9061884955ae08f60d855d785c7ace5752496660bbd9c455e84d3e45` |
| `contracts/fixtures/core/index.json` | `e178cbc42ed02bc81d3780ddab1373463b74dbf0783d645f9c636afa3d291777` |

No additional open finding arose from this bounded resolution recheck. This closes the four contract-review findings for this snapshot only. Parent freeze/integration decisions remain separate. OAuth stays deliberately disabled pending its recorded handoff decision; real HTTP, cookies, authorization, persistence/concurrency, browser/device, production artwork and release acceptance remain untested here. Only this review document changes in the separate resolution commit.
