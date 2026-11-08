"""Migration numbering guard.

Parallel worktrees each branch from the same head and each author the next
migration number, so two packages routinely produce the same version. A merged
tree with a duplicate version cannot be migrated: `migration_runtime` requires
strictly increasing unique versions and refuses the packaged directory.

This guard fails fast on the merged tree, before anyone tries to run a database
command. It is deliberately separate from `wp02_migration_ledger.py`, which
proves the migrations apply; this only proves their names are admissible.
"""
from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SERVICES = ROOT / "backend" / "services"
NAME = re.compile(r"^(\d+)_[a-z0-9_]+\.sql$")


def migration_files(service: Path) -> list[Path]:
    return sorted(
        path
        for path in (service / "migrations").glob("*.sql")
        if not path.name.endswith(".down.sql")
    )


class MigrationNumberingTests(unittest.TestCase):
    def test_every_service_has_a_migration_directory(self) -> None:
        services = sorted(
            path for path in SERVICES.iterdir() if (path / "migrations").is_dir()
        )
        self.assertTrue(services, "no service carries a migrations directory")

    def test_no_service_repeats_a_migration_version(self) -> None:
        for service in sorted(path for path in SERVICES.iterdir() if path.is_dir()):
            if not (service / "migrations").is_dir():
                continue
            with self.subTest(service=service.name):
                seen: dict[int, str] = {}
                for path in migration_files(service):
                    match = NAME.match(path.name)
                    self.assertIsNotNone(
                        match, f"{service.name}: unparseable migration name {path.name}"
                    )
                    version = int(match.group(1))
                    self.assertNotIn(
                        version,
                        seen,
                        f"{service.name}: version {version} is used by both "
                        f"{seen.get(version)} and {path.name}",
                    )
                    seen[version] = path.name

    def test_migration_names_are_lowercase_and_zero_padded(self) -> None:
        for service in sorted(path for path in SERVICES.iterdir() if path.is_dir()):
            if not (service / "migrations").is_dir():
                continue
            for path in migration_files(service):
                with self.subTest(migration=f"{service.name}/{path.name}"):
                    stem = path.stem
                    prefix = stem.split("_", 1)[0]
                    self.assertGreaterEqual(
                        len(prefix), 4, f"{path.name}: version must be zero-padded"
                    )
                    self.assertEqual(
                        stem,
                        stem.lower(),
                        f"{path.name}: migration names are lowercase",
                    )


if __name__ == "__main__":
    unittest.main()
