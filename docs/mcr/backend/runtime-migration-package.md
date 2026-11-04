# Concert runtime migration package

The Concert migration API accepts an explicitly managed
`CONCERT_MIGRATION_DIRECTORY` for `--migrate`, `--verify`, and serving. Without
the variable it validates the compile-time manifest directory. SQL is loaded
from the filesystem at runtime; it is not embedded in the executable.

Source: `9a3c008b6914146d1c38d5de29010dc91ac2d4eb`, restricted to
`src/config/mod.rs`, the migration-command lines of `src/main.rs`, and the
signature accommodation in `tests/participant_cancellation_support/host.rs`
under `backend/services/concert-service/`. Other feature composition is not
part of this intake. The helper counterpart is published at
`521a8075461416f0a696a935ad0a91b363c00491`.

An override must name an absolute directory without dot/parent components or
symlink ancestors. Every entry must be a regular nonsymlink file, and at least
one SQL file is required. Validation precedes opening the database. The owning
launcher must additionally retain the exact source-bound SQL inventory and
check SHA-384 checksums during the allocation; path validation alone cannot
freeze mutable files. An old executable cannot gain the API by receiving a new
environment variable.

## Actual qualification

The owner built source `cfc49f4eec500d13ce62dd200ccf3efe6b8b1b67` with Rust
1.88.0 and subsequently used the retained production-main executable in fresh
`SOCIAL-RUNTIME-PACKAGE-COPIED-20261104T1910Z`. Both commands, `--migrate` and
`--verify`, passed against each exact 47- and 50-migration Git package in one
owned PostgreSQL cluster. All versions and SHA-384 checksums matched. Four path
guards, three moderation denials, two closed-input cases, and two accounting
regressions also passed. The earlier artifact-size refusal remains retained.

- Main SHA-256: `cfd44f339cb3a59eed7c41560f3984422bdde3508deb6ed524bc6312e8de200d`.
- Native report SHA-256: `9f4179dc2f1dc87d9ebad7a65e5be81161bf97a27bb7aae27d9374d2eb6e8f9a`.
- Build-evidence SHA-256: `910bdc8548801098354f0707595476af5611eb68ed2c7613056b4d0562023a08`.
- Owner evidence: `/private/tmp/mcr-social-copied-pjvn8yqd/`.
- Actual acceptance: `2026-11-04T19:08:57.938256Z`.
- Actual FULL release: `2026-11-04T19:09:23.407184Z`; no remaining processes,
  PostgreSQL directories, target access, or active allocation.

The focused launcher source checks previously passed 19 cases. This packet
qualifies runtime SQL loading, not the current combined master application,
real C03/World entry, Wall admin, current Enrollment Auth, or IN04. Those
combined flows remain `NOT_RUN` for this change.
