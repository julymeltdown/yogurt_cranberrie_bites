# Guarded bootstrap runtime — bounded integration

Concert now supports explicit `CONCERT_GRPC_ADDR` startup. Without it, the
existing health-only mode remains available. Local serving binds loopback;
production requires a non-loopback, nonzero address, verified TLS, Gateway-only
caller credentials and Concert actor-delegation verification. No listener opens
before security settings and the product release are validated.

All six product files are mandatory: `CONCERT_BOOTSTRAP_FILE`,
`CONCERT_AVATAR_MANIFEST_FILE`, `CONCERT_MAP_MANIFEST_FILE`,
`CONCERT_REVENGE_MANIFEST_FILE`, `CONCERT_PARADE_MANIFEST_FILE`, and
`CONCERT_DANGER_MANIFEST_FILE`. The executable calls the product constructor
even in local mode. Synthetic art cannot enable the product runtime. Bootstrap
capabilities default off; this does not authorize mutations or world transport.

Rust 1.88.0, locked/offline, jobs=2, debug=0 checks exited 0: Concert library
(2), bootstrap (4 ordinary; 2 explicitly ignored native tests), authority (3),
runtime settings (2), and existing configuration (2). The ignored tests then
ran in a fresh PostgreSQL 16.14 fixture, both passing. The real executable
refused the synthetic product release with exit 1, a safe diagnostic and no
listener. Exact ledger 1/2/3/4/9, reapply, C03 validation, restart and owned
cleanup passed. See [native evidence](bootstrap-native.json).

Reproduction after the recorded Cargo artifact build:

```sh
python3 backend/scripts/mcr/bootstrap_smoke.py \
  --build-metadata /tmp/mcr-delivery-native-pinned-20261104/artifacts.jsonl \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin
```

Exit 0. Executables were copied from exact successful Cargo JSON artifacts and
hashed before transferring the build lease. The native fixture deliberately
uses a synthetic in-process RPC release; it is not a positive real-product
startup or live Gateway-to-production proof. No approved production release
is supplied, so that positive startup, real two-account world and IN01 remain
NOT_RUN. Startup code and the harness were validated in the recorded dirty
integration snapshot, before this documentation/implementation commit.
