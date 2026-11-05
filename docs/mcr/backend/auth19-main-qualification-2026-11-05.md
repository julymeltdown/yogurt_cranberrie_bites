# Auth19 main qualification: 2026-11-05

Status: main executable build qualified; real Enrollment and seven-service startup `NOT_RUN`.

The Product/API developer built `auth-service` at source `6504448fa682b10c53b3f6cd3e0aeef4d75b1483` under `ROOT-AUTH19-MAIN-FINITE-20261104T2353Z`. The clean source contains nineteen Auth migrations, including Enrollment. Its production Auth source matches the already qualified Enrollment operator source `dee64fd1ac868f4dffbea49c0e75ca8e98727f3a`; that operator's test differs and remains a separate executable and receipt.

```sh
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 \
cargo +1.88.0 build --manifest-path backend/Cargo.toml --locked --offline \
  -j 2 --config profile.dev.package.contracts-build.opt-level=2 \
  --message-format=json -p auth-service --bin auth-service
```

Cargo exited 0 in 12.64 seconds. The copied main executable is 47,663,312 bytes, mode `0500`, SHA256 `8e5893d72f661d0d00eefddf4db5ff733d1afffc24bae787865bff7968d7cc35`. The JSON compiler artifact identifies the intended Auth package, `src/main.rs`, `test: false`, and retained-target executable. No service or test executable was launched.

| Retained receipt | SHA256 |
| --- | --- |
| `readset.json` — 2,503 input pins | `0e31694b340f1ebc137338cbd42572aeaf62b60772ca0c48f1e3680ccc6582f3` |
| `source-pins.json` | `f2cf426b8aa2587b58c5f9189bfbfb069ca6dc6b17a3bfba742ddc18bd6d0936` |
| `auth-main-1-receipt.json` | `ac05067546fdad520b337e319f4988f201cd9c086da88726fa318c83a0c80ffd` |
| `acceptance.json` | `dac9c0c2b8fc3425bb7eb09ac0bbe9e04bcee944713d0a291d61b19a8c800e75` |
| `FULL.json` | `568a13c95adffd8c893f294953ba312fe65f71176f0443f41c177596e7ebdd9f` |

Raw controls, compiler records and the immutable executable remain in the local directory identified by basename `mcr-root-auth19-main-h3vjxn9r`. Machine-local paths are excluded from this public document.

The prepared `2340Z` request never accepted a lease. Before acceptance, only its offer identifier was changed to the actual `2353Z` offer; the original plan/request bytes were preserved. Actual plan SHA256 is `824e6d18b2ef381a19d2c53b9b4f5334190046c9efa82ec8fbc4d393313c4146`; request SHA256 is `3188c022a507f8157cb3bc3cabc16cedf76c2c534e2648c77210ddf174f61d34`.

An earlier admission attempt stopped before acceptance or target access when it found another developer's unallocated compiler. That owner stopped and reaped its processes and removed only its private target; the allocator confirmed the custody exception resolved. Its source/test observations remain resource-procedure unqualified and do not qualify this build.

Actual acceptance epoch: `1793836925.4206662`. Target and full release epoch: `1793836942.47759`. Both live guards and terminal source/handle checks passed. All owned children, workflows, remaining growth and target custody were released; the allocator, World and Gift received the release before documentation.

The fresh retained-target baseline was 19,484,332,032 bytes. The complete carry floor was 32 GiB, WHOLE 40 GiB and NEW 2 GiB. Observed peaks were 9 processes, 1,171,816,448 bytes RSS, 49,512,448 bytes NEW and 34,409,250,816 bytes WHOLE; minimum free space was 15,340,716,032 bytes. Fixed compile/copy/work/full bounds were 600/660/720/900 seconds, including 180 seconds for cleanup.

The previously missing Auth19 main input now exists alongside the qualified Concert and Gateway mains. Actual PostgreSQL migration receipts, same-stack policy publication, mail verification, three real principals, operator-role application, Settings, private runtime descriptor, Wall restart, default App and multiuser journeys remain `NOT_RUN`. The three-principal operator profile does not satisfy QA04's seven-principal requirement. The integration goal remains `usageLimited`; this build neither resets nor completes it.
