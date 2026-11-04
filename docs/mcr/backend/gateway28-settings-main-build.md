# Gateway28 Settings main build

Status: developer-owned ONE-main compilation passed. Gateway execution, route tests, PostgreSQL, browser, the same-cohort multiuser journey and IN04 remain `NOT_RUN` in this round. This binary contains the exact Settings row; it is neither the earlier Gateway27 binary nor a BE38 policy executable.

## Source and provenance

- Source commit: `1ab1cc579724851f7a222d1083551573ccbd8595`; parent `dee64fd1ac868f4dffbea49c0e75ca8e98727f3a`.
- Settings path: `backend/services/api-gateway/src/adapters/in/http/concert_product/feature_routes.rs`, SHA256 `e6ec4a1bcadbd9d8e93f3993795d42dd28fcae988137df54df27dc4eee3bbbe7`.
- Exact operation: `getConcertSettings`, bodyless GET `/api/concert/events/{event_id}/admin/settings`; no query, Content-Type or idempotency key. Existing canonical UUID admission and protected errors are retained. This is a transport row, not an operator grant.
- Frozen 1,954 source/SQL inputs: SHA256 `a43470db125da6fae5fe008422253b304a6f4053007f8caa3289fd917055b024`. The original 1,953-input readset `c77a872dc2b278713a178db34450079e19a8703ec48119e15e01d585f587de51` is retained unchanged.
- The one added pre-ACCEPT production input was the already-tracked `docs/mcr/contracts/avatar-motion-v2/review/visual-review.json`, SHA256 `27dd1f7f1d31b792f4da415cf6daf6eafc97e91023443a27ac70480f75ce5956`. No source or test fixtures changed.

## Developer validation

```sh
cargo +1.88.0 build --manifest-path backend/Cargo.toml --locked --offline -j 2 \
  --config profile.dev.package.contracts-build.opt-level=2 \
  -p api-gateway --bin api-gateway --message-format=json
```

Exit **0**, 18.83 seconds. Debug info and incremental compilation were disabled; Rust flags and wrappers were unset. The two Auth/Gateway build scripts and the Auth/Gateway application library roots received audited byte-identical mtime refreshes to avoid stale warm artifacts. The source revision and all frozen inputs still matched after the build.

Actual allocation `GATEWAY28-MAIN-MEDIA-20261104T2125Z` used the existing warm target only. Acceptance: 2026-11-04 21:25:22.523084 UTC. Both guards passed; peak 8 processes / 1,453,015,040 bytes RSS. Inclusive NEW high-water was 58,933,248 bytes; whole charge 6,501,384,192 bytes, including a nonshrinking 6 GiB carry. The build cap was 1 GiB NEW / 12 GiB WHOLE, 40 processes / 8 GiB RSS.

Target release: **21:25:51.791025 UTC**, SHA256 `1529fc8724a5f405f9632ac0ec6003d17a76d3d72be7160f882940ab030aba85`, compiler/readers/handles/guardians zero. It was delivered directly to Root1fc and allocator21d before cleanup or documentation. No target reentry occurred. FULL: **21:26:26.6270678 UTC**, SHA256 `0b6f954682258f1c67601611591e2e59ec8a531d4ca12e14437032a8970ad19c`, all owned resources and remaining NEW zero.

## Immutable artifact and handoff

The private build packet is `/private/tmp/mcr-gateway207-settings-28-enfm_kd9/qualified-artifact.json`, SHA256 `bc87636e9d09c5f9136ae804b10e392e0a49cd0544dca31ada9c1d7481d00a77`. It includes the exact command, tool/controller/source pins, 19 Auth and 17 Gateway SQL pins, mtime audit, copy identity, guard and release receipts.

Executable: `/private/tmp/mcr-gateway207-settings-28-enfm_kd9/api-gateway-frozen`, 58,051,312 bytes, mode 0555, SHA256 `ece5a920fac003fadec08f13175cb093c3b1be096ed82ac136e31c239ec08cf8`. It was copied and hashed before target release and was never launched by developer207. Root1fc owns use in the actual existing serving window; FE204 received the qualified packet. A future runtime window requires its own resource allocation and actual serving/cohort evidence. Private temporary paths are local evidence locators, not durable or portable download URLs.

Publication routes directly to the standing publishing developer412e. No king, SL or separate review/acknowledgment stage applies.
