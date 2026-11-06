"""Static/failure-path guards; the native PG run remains the admission proof."""
from __future__ import annotations

import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from contextlib import redirect_stderr

import world_admission_smoke as smoke


class GuardChecks(unittest.TestCase):
    def test_migration_reservation_and_tables_are_exact(self):
        names = [path.name for path in smoke.migration_paths()]
        self.assertIn("0005_world_admission.sql", names)
        self.assertGreater(len(names), 5, "full production schema is required")
        self.assertEqual(
            set(smoke.TABLES),
            {
                "concert_world_rooms",
                "concert_world_owner_leases",
                "concert_world_tickets",
                "concert_world_memberships",
            },
        )

    def test_relative_binary_is_rejected(self):
        report = {}
        with self.assertRaises(smoke.Failure):
            with patch.object(smoke.migration, "executable") as executable:
                smoke.run(Path("relative"), report)
        executable.assert_not_called()

    def test_receipt_hashes_current_gateway_http_sources(self):
        bridge = [
            "backend/services/api-gateway/tests/support/concert_world_client.rs",
            "backend/services/api-gateway/tests/support/concert_world_http.rs",
        ]
        self.assertTrue(all(path in smoke.GATEWAY_SOURCES for path in bridge))
        self.assertNotIn(
            "backend/services/concert-service/src/world/mod.rs",
            smoke.GATEWAY_SOURCES,
        )

    def test_existing_report_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory) / "existing.json"
            report.write_text("preserve")
            with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                smoke.main(
                    [
                        "--binary",
                        "/fake/world",
                        "--report",
                        str(report),
                    ]
                )
            self.assertEqual(report.read_text(), "preserve")

    def test_cleanup_failure_fails_and_retains_owned_directory(self):
        with patch.object(smoke, "exercise"), patch.object(
            smoke.migration, "Cluster"
        ) as cluster:
            cluster.return_value.root = Path("/retained-owned-world")
            cluster.return_value.cleanup.side_effect = OSError("fixture")
            report = {}
            with patch.object(smoke.migration, "executable", return_value=Path("/fake")):
                smoke.run(Path("/fake"), report)
        self.assertEqual(report["result"], "FAIL")
        self.assertEqual(report["cleanup"], "FAIL")
        self.assertEqual(
            report["owned_directory"], str(Path("/retained-owned-world"))
        )


if __name__ == "__main__":
    unittest.main()
