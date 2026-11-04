#!/usr/bin/env python3
"""Pure observer/guard regressions. Fixture dictionaries are not runtime evidence."""
import copy
import json
from pathlib import Path
import stat
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
import qa09_backend as qa

SCOPE = "bb0f404c-61ce-4840-a907-e34788566425"
KEY = b"private-unit-only-correlation-key!"


def snapshot(table, rows):
    return {"kind": "READ_ONLY_OWNER_SNAPSHOT", "owner": "concert",
            "sourceRevision": "a" * 40, "runBinding": "b" * 64, "scopeBinding": "c" * 64,
            "lineageBinding": "e" * 64, "verifiedMigrationCount": 50,
            "capturedAt": "2026-11-04T12:00:00+00:00",
            "tables": {table: {"state": "IN_PROGRESS", "querySha256": "d" * 64, "rows": rows}}}


class ObserverTests(unittest.TestCase):
    def test_original_cases_start_not_run_and_never_claim_acceptance(self):
        report = qa.plan("e" * 32)
        self.assertEqual(tuple(x["caseId"] for x in report["cases"]), qa.CASES)
        self.assertFalse(report["wholeQa09Accepted"])
        for case in report["cases"]:
            self.assertEqual(case["state"], "NOT_RUN")
            self.assertIsNone(case["producerEvidence"])
            self.assertTrue(all(value is None for value in case["phases"].values()))

    def test_run_id_is_local_closed_and_cannot_include_private_input(self):
        for value in ["user@example.invalid", "F" * 32, "a" * 31, "../secret"]:
            with self.assertRaises(ValueError):
                qa.plan(value)

    def test_queries_are_read_only_bounded_and_reject_scope_injection(self):
        for table in qa.PROJECTIONS:
            sql = qa.query_for(table, SCOPE)
            self.assertTrue(sql.startswith("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;"))
            self.assertIn("LIMIT 1001", sql)
            self.assertTrue(sql.endswith("ROLLBACK;"))
            for forbidden in ["INSERT ", "DELETE ", "UPDATE ", "CREATE ", "DROP "]:
                self.assertNotIn(forbidden, sql)
            for bad in ["';drop table users;--", SCOPE.upper(), "00000000-0000-0000-0000-000000000000"]:
                with self.assertRaises(ValueError):
                    qa.query_for(table, bad)

    def test_private_identity_becomes_only_run_local_hmac(self):
        rows = [{"id": "private-owner-identity", "state": "CLAIMED", "revision": 2}]
        clean = qa.sanitize_rows("reservations", rows, KEY)
        self.assertNotIn("private-owner-identity", json.dumps(clean))
        self.assertEqual(len(next(iter(clean))), 64)
        self.assertNotEqual(clean, qa.sanitize_rows("reservations", rows, b"other-private-run-key-32bytes!!!!"))
        with self.assertRaises(ValueError):
            qa.sanitize_rows("reservations", rows, b"short")

    def test_projection_refuses_payloads_duplicates_types_and_unbounded_rows(self):
        good = {"id": "private", "state": "CLAIMED", "revision": 2}
        bad = [dict(good, payload="secret"), dict(good, state="VERIFIED"),
               dict(good, revision=True), dict(good, revision=-1), {"id": "private"}]
        for row in bad:
            with self.assertRaises(ValueError):
                qa.sanitize_rows("reservations", [row], KEY)
        for rows in [[good, good], [good] * 1001, {}]:
            with self.assertRaises(ValueError):
                qa.sanitize_rows("reservations", rows, KEY)

    def test_comparison_requires_actual_observer_binding_metadata(self):
        for value in [{}, {"kind": "external-json"}, {"kind": "READ_ONLY_OWNER_SNAPSHOT", "owner": "concert"}]:
            with self.assertRaises(ValueError):
                qa.compare(qa.CASES[3], value, value)

    def test_candidate_run_scope_owner_drift_is_not_comparable(self):
        before = snapshot("reservations", {"token": {"state": "CLAIMED", "revision": 1}})
        for field, value in [("sourceRevision", "f" * 40), ("runBinding", "f" * 64),
                             ("scopeBinding", "f" * 64), ("lineageBinding", "f" * 64), ("verifiedMigrationCount", 49), ("owner", "auth")]:
            after = copy.deepcopy(before)
            after[field] = value
            with self.assertRaises(ValueError):
                qa.compare(qa.CASES[3], before, after)

    def test_reversed_or_naive_timestamps_fail_closed(self):
        before = snapshot("reservations", {})
        for value in ["2026-11-04T11:59:59+00:00", "2026-11-04T12:00:00"]:
            after = copy.deepcopy(before)
            after["capturedAt"] = value
            with self.assertRaises(ValueError):
                qa.compare(qa.CASES[3], before, after)

    def test_projection_drift_is_rejected(self):
        before = snapshot("reservations", {"token": {"state": "CLAIMED", "revision": 1}})
        after = copy.deepcopy(before)
        after["tables"]["reservations"]["querySha256"] = "e" * 64
        with self.assertRaises(ValueError):
            qa.compare(qa.CASES[3], before, after)

    def test_empty_or_unavailable_rows_cannot_prove_persistence(self):
        before = snapshot("reservations", {})
        self.assertEqual(qa.compare(qa.CASES[3], before, before)["state"], "NOT_RUN")
        before["tables"]["reservations"] = {"state": "NOT_RUN"}
        self.assertEqual(qa.compare(qa.CASES[3], before, before)["comparedTables"], 0)

    def test_lost_or_reverted_committed_pickup_is_detected(self):
        before = snapshot("reservations", {"token": {"state": "CLAIMED", "revision": 3}})
        for rows in [{}, {"token": {"state": "READY", "revision": 3}}]:
            after = snapshot("reservations", rows)
            self.assertIn("COMMITTED_STATE_CHANGED_OR_LOST", qa.compare(qa.CASES[3], before, after)["violations"])

    def test_inventory_overflow_and_lost_claimed_quantity_are_detected(self):
        row = {"quantity_total": 5, "reserved_unclaimed": 1, "quantity_claimed": 2}
        before = snapshot("inventory", {"token": row})
        for changed, expected in [(dict(row, reserved_unclaimed=4), "INVENTORY_OVERFLOW"),
                                  (dict(row, quantity_claimed=1), "COMMITTED_PICKUP_COUNT_LOST")]:
            result = qa.compare(qa.CASES[3], before, snapshot("inventory", {"token": changed}))
            self.assertIn(expected, result["violations"])
        legitimate = snapshot("inventory", {"token": dict(row, reserved_unclaimed=0, quantity_claimed=3)})
        self.assertEqual(qa.compare(qa.CASES[3], before, legitimate)["violations"], [])

    def test_outbox_delivered_row_loss_is_detected_without_dedup_claim(self):
        before = snapshot("outbox", {"token": {"state": "DELIVERED", "attempts": 2}})
        result = qa.compare(qa.CASES[2], before, snapshot("outbox", {}))
        self.assertIn("COMMITTED_STATE_CHANGED_OR_LOST", result["violations"])
        self.assertFalse(result["wholeCaseAccepted"])

    def test_partial_closure_cannot_lose_prior_owner_effect(self):
        before = snapshot("closure_effects", {"token": {"revision": 3}})
        after = snapshot("closure_effects", {})
        self.assertIn("COMMITTED_STATE_CHANGED_OR_LOST", qa.compare(qa.CASES[5], before, after)["violations"])

    def test_deletion_success_does_not_regress_to_pending(self):
        before = snapshot("deletion_steps", {"token": {"state": "SUCCEEDED", "attempts": 2}})
        before["owner"] = "auth"
        after = copy.deepcopy(before)
        after["tables"]["deletion_steps"]["rows"]["token"]["state"] = "PENDING"
        self.assertIn("COMMITTED_STATE_CHANGED_OR_LOST", qa.compare(qa.CASES[4], before, after)["violations"])

    def test_clean_rows_and_revision_checks_are_observations_only(self):
        before = snapshot("reservations", {"token": {"state": "READY", "revision": 3}})
        result = qa.compare(qa.CASES[3], before, before)
        self.assertEqual(result["state"], "IN_PROGRESS")
        self.assertFalse(result["wholeCaseAccepted"])
        after = snapshot("reservations", {"token": {"state": "READY", "revision": 2}})
        self.assertIn("REVISION_REGRESSED", qa.compare(qa.CASES[3], before, after)["violations"])

    def test_capture_refuses_attaching_foreign_cluster_or_url(self):
        with self.assertRaises(ValueError):
            qa.capture(object(), "concert", "mcr_concert", SCOPE, KEY)

    def test_restart_refuses_foreign_cluster_without_process_or_sql_access(self):
        with self.assertRaises(ValueError):
            qa.restart_owner_cluster(object(), KEY)

    def test_restore_checkpoint_must_pin_both_new_orchestration_sources(self):
        pins = qa.script_pins()
        qa.verify_checkpoint_scripts({"qa09_source_sha256": pins})
        for changed in [{}, {"extra": "a" * 64}, {key: "f" * 64 for key in pins}]:
            with self.assertRaises(ValueError):
                qa.verify_checkpoint_scripts({"qa09_source_sha256": changed})

    def test_report_is_exclusive_private_and_preserves_existing_evidence(self):
        with tempfile.TemporaryDirectory(prefix="mcr-qa09-unit-") as directory:
            report = Path(directory) / "report.json"
            qa.write_report(report, {"state": "NOT_RUN"})
            self.assertEqual(stat.S_IMODE(report.stat().st_mode), 0o600)
            saved = report.read_bytes()
            with self.assertRaises(ValueError):
                qa.write_report(report, {"state": "ACCEPTED"})
            self.assertEqual(report.read_bytes(), saved)
            link = Path(directory) / "link.json"
            link.symlink_to(report)
            with self.assertRaises(ValueError):
                qa.write_report(link, {})


if __name__ == "__main__":
    unittest.main()
