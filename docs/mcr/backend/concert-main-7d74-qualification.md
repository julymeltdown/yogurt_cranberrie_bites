# Concert main qualification: 2026-11-05

Status: main executable build qualified; startup and application journeys NOT_RUN.

The Product/API developer built source `7d74b7fbf397688f3ae41dd7fa5347d3bcc50aa4` under the finite `ROOT-CONCERT-RETAINED-WHOLE-RETRY-20261104T2140Z` offer after Gift's actual target and full release. The source checkout remained clean throughout compilation. This source includes the Settings same-connection fence and OwnAccess fixes from owner 205; it does not include the future BE38/BE39/BE42 producer increments.

Command, with the already retained target supplied through `CARGO_TARGET_DIR`:

```sh
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 \
cargo +1.88.0 build --manifest-path backend/Cargo.toml --locked --offline \
  -j 2 --config profile.dev.package.contracts-build.opt-level=2 \
  --message-format=json -p concert-service --bin concert-service
```

Cargo exited 0 in 34.56 seconds. The copied `concert-service` executable is 70,724,752 bytes, mode `0500`, SHA256 `9abf5914bc9314cb93be749a90ed95bd9ecf2f4bebe3bbde1cb08b54e8aa77f2`. Its Cargo manifest and executable origins were checked against the intended package and retained target. No executable was launched.

| Receipt | SHA256 |
| --- | --- |
| `readset.json` — 2,518 immutable input pins | `8cfa104b986709d5fa83686095468c95334fecadf146e26dab66ec912bd44e84` |
| `source-pins.json` — tracked backend/contract inputs | `4be122573aada099db6871e7c341ad7c8f511587e41ec61d6a8f4490b939389c` |
| `acceptance.json` | `375137cfbf4994931a5bd638cb7e19111d3126fa1bb163db5fdf560a82262db4` |
| `concert-main-1-receipt.json` | `58711aca33e9cb8989cec2522d8447be182bfa60dfaded618b4ef91ebd6dbe9d` |
| `FULL.json` | `8cf8b3c2d1b9377e6ade4c33b8640e32b5bf73f7e01b4907564e9c2269434582` |

The local receipt directory is identified by basename `mcr-root-main-retry-vqt08b4q`; raw controls and compiler records remain there. They are not copied into the public repository because they contain machine-local paths.

Acceptance epoch: `1793834702.7365718`. Actual target/full release epoch: `1793834742.236775`. Both independent live guards passed; the terminal target-handle/source checks passed. All owned children, workflows, remaining growth and target custody were released. The allocator and next target owner received the release before documentation work.

The retained target baseline was 19,541,331,968 bytes. The complete carry floor was 32 GiB within a 40 GiB WHOLE allowance; NEW was bounded at 2 GiB. Observed peaks: 9 owned processes, 2,206,826,496 bytes RSS, 75,206,656 bytes NEW and 34,434,945,024 bytes WHOLE; minimum free space was 17,421,471,744 bytes. The original 600/660/720/900-second compile/copy/work/full deadlines and 180-second cleanup limit remained fixed.

This receipt does not qualify database migrations, real service startup, provisioning, signed operator roles, private cohort binding, snapshot/restart, browser/SSR, IN04, any actual QA05 case, or future moderation/lifecycle producers. Those remain developer-owned work requiring their actual runtime inputs and separate resource admission. The existing integration goal metadata remains `usageLimited`; this build does not reset or complete that goal.
