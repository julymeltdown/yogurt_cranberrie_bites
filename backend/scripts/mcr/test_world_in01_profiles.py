"""Bounded offline producer regressions. No Auth, PG, native child or listener runs."""
import copy
import json
import re
import tempfile
from pathlib import Path
from types import SimpleNamespace as NS
import unittest
from unittest.mock import AsyncMock, Mock, patch
from uuid import uuid4

import world_in01_live as live
import world_in01_prepare as prep
from test_world_capacity_probe import configuration
from test_world_live_failure_probe import FixtureNetwork
from world_capacity_support.report import Failure
from world_capacity_support.scenarios import Runner
from world_capacity_support.transport import Contracts


def arguments(profile="qa05"):
    return NS(qualification_profile=profile, allocation_id="FIXTURE-NOT-ACTUAL-ALLOCATION",
              provision_timeout_seconds=120, probe_timeout_seconds=300, measurement_seconds=20,
              input_hz=15, arrival_concurrency=4, restart_world=False,
              qa08_provisioning_budget="signup=400,login=200" if profile == "qa08" else None,
              whole_timeout_seconds=600, whole_byte_limit=1073741824, retained_carry_bytes=309342208,
              retain_public_projection=False,
              frontend_source_revision=None,
              browser_origin="http://localhost:5173", world=True, hold_seconds=0)


def handoff(profile="qa05"):
    args = arguments(profile)
    config = configuration(profile == "qa08")
    config["rooms"] = [{"room_id": r["room_id"], "capacity": cap}
                       for r, cap in zip(config["rooms"], prep.producer_profile(profile)["capacities"])]
    principals = [(str(uuid4()), Path("/not-read"), p["bearer"]) for p in config["principals"]]
    return args, config, principals


class ProfileTests(unittest.TestCase):
    def test_default_in01_and_original_cohorts(self):
        self.assertEqual(prep.producer_profile(), {"name": "in01", "principals": 2, "capacities": (2,)})
        self.assertEqual(prep.producer_profile("qa05")["capacities"], (2, 2))
        self.assertEqual(prep.producer_profile("qa08")["principals"], 200)
        for value in ("arbitrary", [], 0):
            with self.assertRaises(prep.auth.CheckFailure): prep.producer_profile(value)

    def test_provisioning_calls_real_principal_route_and_stops_on_duplicate(self):
        stack, browser = object(), object()
        first, second = (str(uuid4()), None, "token-a"), (str(uuid4()), None, "token-b")
        with patch.object(prep, "principal", side_effect=[first, second]) as create:
            self.assertEqual(prep.provision_principals(stack, browser, "qa05"), [first, second])
        self.assertEqual(create.call_count, 2)
        with patch.object(prep, "principal", side_effect=[first, first]):
            with self.assertRaisesRegex(prep.auth.CheckFailure, "distinct-canonical"):
                prep.provision_principals(stack, browser, "qa05")

    def test_normal_auth_refusal_is_not_retried_or_bypassed(self):
        with patch.object(prep, "principal", side_effect=prep.auth.CheckFailure("principal-1-signup:status")) as create:
            with self.assertRaises(prep.auth.CheckFailure): prep.provision_principals(None, None, "qa08")
        self.assertEqual(create.call_count, 1)

    def test_provisioning_deadline_stops_before_next_account(self):
        with patch.object(prep.time, "monotonic", side_effect=[0, 0, 2]), patch.object(prep, "principal", return_value=(str(uuid4()), None, "token")) as create:
            with self.assertRaisesRegex(prep.auth.CheckFailure, "principal-provision-deadline"):
                prep.provision_principals(None, None, "qa08", 1)
        self.assertEqual(create.call_count, 1)

    def test_real_principal_sequence_does_not_persist_token(self):
        user, token = str(uuid4()), "PRIVATE-TEST-TOKEN"
        browser = Mock()
        captured = {}
        def request(label, path, payload):
            captured.update(payload)
            if path.endswith("signup") or path.endswith("verify"):
                return {}, {"userId": user}
            return {"set-cookie": "refresh=fake; HttpOnly; SameSite=Lax; Path=/"}, {"userId": user, "accessToken": token, "refreshToken": ""}
        browser.request.side_effect = request
        mail = Mock()
        mail.is_symlink.return_value = False
        mail.stat.return_value = NS(st_mode=0o600)
        mail.read_bytes.side_effect = lambda: json.dumps({"kind": "VERIFY_EMAIL", "payload": {"email": captured["email"], "verification_code": "fixture-code"}}).encode()
        stack = NS(cluster=NS(root=Path("/fixture-only"), assert_owned=Mock()),
                   mail=NS(directory=NS(glob=lambda _: [mail])))
        with patch.object(prep, "private_json") as persist:
            item = prep.principal(stack, browser, 1)
        self.assertEqual(item[2], token)
        self.assertNotIn(token, json.dumps(persist.call_args.args[1]))
        self.assertEqual(persist.call_args.args[1]["access_token"], "MEMORY_ONLY")
        self.assertEqual([c.args[1] for c in browser.request.call_args_list], ["/api/auth/email/signup", "/api/auth/email/verify", "/api/auth/login"])

    def test_operator_seed_only_changes_room_configuration(self):
        bootstrap = configuration()["bootstrap"]
        for profile, capacities in (("in01", [2]), ("qa05", [2, 2]), ("qa08", [50]*4)):
            with self.subTest(profile=profile):
                statements = []
                def sql(database, query):
                    statements.append(query)
                    if query.startswith("BEGIN; ALTER"): return "f"
                    if query.startswith("SELECT count"): return "0"
                    return "t"
                cluster = NS(root=Path("/fixture-only"), env={}, tools={"createdb": Path("/not-executed")},
                             assert_owned=Mock(), client_env=lambda _: {}, url=lambda _: "private-db-fixture", sql=sql)
                with patch.object(prep.dev.migration, "executable", side_effect=lambda p:p), \
                     patch.object(prep.dev.migration, "execute"), \
                     patch.object(prep.dev.migration, "migration_manifest", return_value={}), \
                     patch.object(prep.dev.migration, "check_ledger", return_value=37), \
                     patch.object(prep.dev.runtime, "digest", return_value="0"*64), \
                     patch.object(Path, "read_bytes", return_value=json.dumps(bootstrap).encode()), \
                     patch.object(Path, "read_text", return_value="SCHEMA-FIXTURE"), \
                     patch.object(prep, "release_inputs", return_value={}), \
                     patch.object(prep, "private_json") as persist:
                    prep.seed_concert(NS(cluster=cluster), Path("/not-executed"), {}, False, profile)
                rooms = json.loads(persist.call_args.args[1]["CONCERT_WORLD_ROOMS_JSON"])
                self.assertEqual([r["capacity"] for r in rooms], capacities)
                self.assertEqual(len({r["room_id"] for r in rooms}), len(rooms))
                insert = next(q for q in statements if "INSERT INTO concert_events" in q)
                self.assertEqual(insert.count("INSERT INTO concert_world_rooms"), len(rooms))
                for table in ("users", "concert_participants", "concert_event_admissions", "concert_avatar_settings"):
                    self.assertNotIn("INSERT INTO " + table, insert)

    def test_in_memory_handoff_balances_original_capacity(self):
        for profile in ("qa05", "qa08"):
            args, config, principals = handoff(profile)
            result = live.qualification_config(args, config, {"event": config["event_id"], "rooms": config["rooms"]}, principals,
                                               config["bootstrap"], config["ws_url"])
            self.assertEqual(len(result["principals"]), len(principals))
            self.assertEqual(result["principals"][0]["bearer"], principals[0][2])
            if profile == "qa08":
                self.assertEqual([sum(p["room_index"] == r for p in result["principals"]) for r in range(4)], [50]*4)
                self.assertEqual(result["schedule"]["http_interval_seconds"], .12)
            else:
                self.assertIn("room_switch", result["faults"])

    def test_invalid_flags_fail_before_starting_resources(self):
        for key, value in (("world", False), ("hold_seconds", 2), ("allocation_id", None), ("input_hz", float("nan"))):
            args = arguments()
            setattr(args, key, value)
            with self.assertRaises((prep.auth.CheckFailure, Failure)): live.validate_qualification_arguments(args)
        args = arguments("qa08")
        args.restart_world = True
        with self.assertRaises(prep.auth.CheckFailure): live.validate_qualification_arguments(args)
        args.restart_world = False
        args.qa08_provisioning_budget = None
        with self.assertRaisesRegex(prep.auth.CheckFailure, "qa08-explicit-normal-budget"):
            live.validate_qualification_arguments(args)

    def test_default_cli_stays_in01(self):
        with patch("sys.argv", ["world_in01_live.py", "--bin-dir", "/fixture", "--concert-bin", "/fixture/concert", "--report", "/fixture/report"]), patch.object(live, "run", return_value=0) as execute:
            self.assertEqual(live.main(), 0)
        self.assertEqual(execute.call_args.args[0].qualification_profile, "in01")

    def test_reused_binary_release_fence_rejects_map6_before_resource_work(self):
        bootstrap = configuration()["bootstrap"]
        bootstrap["map_manifest"].update(map_id="mcr_reference_scene_r3", map_revision=4)
        bootstrap["avatar_manifest"].update(manifest_id="mcr_selected_static_20261103", manifest_version=1)
        with patch.object(Path, "read_bytes", return_value=json.dumps(bootstrap).encode()), patch.object(prep, "release_inputs") as pins:
            live.qualified_release()
        pins.assert_called_once()
        bootstrap["map_manifest"]["map_revision"] = 6
        with patch.object(Path, "read_bytes", return_value=json.dumps(bootstrap).encode()), patch.object(prep, "release_inputs") as pins:
            with self.assertRaisesRegex(prep.auth.CheckFailure, "qualification-release-pin"): live.qualified_release()
        pins.assert_not_called()

    def test_probe_object_callback_and_oracle_are_not_serialized(self):
        args, config, principals = handoff()
        config = live.qualification_config(args, config, {"event": config["event_id"], "rooms": config["rooms"]}, principals, config["bootstrap"], config["ws_url"])
        result = {"result": "COMPLETED", "server_metrics_and_cleanup_oracle": {"status": "OBSERVED_PRODUCER_HOOK", "memberships": 0, "prepared_transactions": 0}}
        async def probe(received, **kwargs):
            self.assertIs(received, config)
            self.assertEqual(await kwargs["observation_hook"](), {"memberships": 0, "prepared_transactions": 0})
            return copy.deepcopy(result)
        with patch("world_capacity_support.scenarios.run", side_effect=probe), patch.object(live, "membership_observation", return_value={"memberships": 0, "prepared_transactions": 0}), patch.object(prep, "private_json") as persist:
            observed = live.qualification_run(config, NS(cluster=object()))
        self.assertEqual(observed["result"], "COMPLETED")
        persist.assert_not_called()
        self.assertNotIn(principals[0][2], json.dumps(observed))
        result["server_metrics_and_cleanup_oracle"]["memberships"] = 1
        with patch("world_capacity_support.scenarios.run", new=AsyncMock(return_value=result)):
            self.assertEqual(live.qualification_run(config, NS(cluster=object()))["failure"], "producer-cleanup-oracle")

    def test_restart_only_uses_direct_child_and_fresh_owned_port_reservations(self):
        child = NS(process=NS(pid=123), stop=Mock(return_value={"exit_code": 0, "forced": False}))
        replacement = NS(process=NS(pid=124), wait_listeners=Mock(), stop=Mock())
        ports = Mock()
        names = ["concert-service:grpc", "concert-service:metrics", "concert-service:world"]
        stack = NS(cluster=NS(assert_owned=Mock()), ports=NS(ports=dict(zip(names, [31001, 31002, 31003]))))
        with patch.object(live, "RestartPorts", return_value=ports), patch.object(live.dev.runtime, "Child", return_value=replacement), patch.object(live.dev.runtime, "http_probe", return_value=(200, b'')):
            records = []
            actual, env = live.restart_owned_concert(stack, child, {"CONCERT_WORLD_OWNER_ID": "old"}, Path("/not-executed"), names, records)
        self.assertIs(actual, replacement)
        self.assertNotEqual(env["CONCERT_WORLD_OWNER_ID"], "old")
        self.assertEqual(ports.reserve.call_count, 3)
        ports.close.assert_called_once()
        self.assertEqual(records[0]["after_pid"], 124)
        child.stop.return_value = {"exit_code": -9, "forced": True}
        with patch.object(live.dev.runtime, "Child") as spawn:
            with self.assertRaises(prep.auth.CheckFailure): live.restart_owned_concert(stack, child, {}, None, names, [])
        spawn.assert_not_called()

    def test_restart_reservation_is_owned_loopback_and_never_reuseport(self):
        handle = Mock()
        with patch.object(live.socket, "socket", return_value=handle):
            ports = live.RestartPorts()
            self.assertEqual(ports.reserve("concert-service:world", 31003), 31003)
            handle.bind.assert_called_once_with(("127.0.0.1", 31003))
            handle.setsockopt.assert_called_once_with(live.socket.SOL_SOCKET, live.socket.SO_REUSEADDR, 1)
            with self.assertRaises(prep.auth.CheckFailure): ports.reserve("concert-service:world", 31003)
            ports.close()
        handle.close.assert_called_once()

    def test_guard_counts_carry_outputs_and_deadline_without_starting_thread(self):
        args = arguments()
        with tempfile.TemporaryDirectory() as directory, patch.object(live.shutil, "disk_usage", return_value=NS(free=20*1024**3)), patch.object(live.subprocess, "run", return_value=NS(stdout="")):
            args.report = Path(directory) / "report.json"
            guard = live.QualificationGuard(args, NS(cluster=None))
            guard.check()
            self.assertGreaterEqual(guard.maximum, args.retained_carry_bytes)
            self.assertIsNone(guard.thread)
            guard.limit = 1
            with self.assertRaisesRegex(prep.auth.CheckFailure, "qualification-whole-byte-cap"): guard.check()
            guard.limit = args.whole_byte_limit
            guard.deadline = 0
            with self.assertRaisesRegex(prep.auth.CheckFailure, "qualification-work-deadline"): guard.check()
            self.assertEqual(guard.close()["cleanup_reserve_seconds"], 240)

    def test_offer_floors_and_absolute_phase_cutoffs_before_children(self):
        args = arguments()
        with tempfile.TemporaryDirectory() as directory, patch.object(live.subprocess, "run", return_value=NS(stdout="")):
            args.report = Path(directory) / "report.json"
            guard = live.QualificationGuard(args, NS(cluster=None))
            with patch.object(live.shutil, "disk_usage", return_value=NS(free=15*1024**3)):
                with self.assertRaisesRegex(prep.auth.CheckFailure, "start-free-space"): guard.start()
            self.assertIsNone(guard.thread)
            with patch.object(live.shutil, "disk_usage", return_value=NS(free=7*1024**3)):
                with self.assertRaisesRegex(prep.auth.CheckFailure, "free-space"): guard.check()
            guard.phase("fixture-phase", 10)
            self.assertLessEqual(guard.phase_deadline, guard.deadline)
            evidence = guard.close()
            self.assertIn("+00:00", evidence["hard_deadline_at"])
            self.assertEqual(evidence["phase_cutoffs"][0]["name"], "fixture-phase")

    def test_process_guard_counts_only_producer_and_owned_descendants(self):
        args = arguments()
        with tempfile.TemporaryDirectory() as directory, patch.object(live.os, "getpid", return_value=100):
            args.report = Path(directory) / "report.json"
            guard = live.QualificationGuard(args, NS(cluster=None))
            with patch.object(live.subprocess, "run", return_value=NS(stdout="100 1\n200 100\n300 200\n999 1\n")):
                guard.process_check()
            self.assertEqual(guard.process_peak, 3)
            with patch.object(live.subprocess, "run", return_value=NS(stdout="\n".join(f"{n} 100" for n in range(200,233)))):
                with self.assertRaisesRegex(prep.auth.CheckFailure, "process-cap"): guard.process_check()

    def test_retained_snapshot_is_actual_validated_private_and_never_flag_flipped(self):
        bootstrap = json.loads((prep.RELEASE / "bootstrap.json").read_bytes())
        bootstrap["capabilities"]["world"] = {"available": True, "reason": None}  # Synthetic positive fixture only.
        raw = json.dumps(bootstrap, indent=2).encode() + b'\n'
        principal = (bootstrap["participant"]["user_id"], None, "PRIVATE-TEST-TOKEN")
        with tempfile.TemporaryDirectory() as directory, patch.object(live, "public_json", return_value=raw):
            metadata = live.retain_public_projection(None, {}, bootstrap["event"]["event_id"], principal, Path(directory))
            path = Path(directory) / metadata["filename"]
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            projection = json.loads(path.read_bytes())
            self.assertEqual(projection["fidelity"], "NOT_BYTE_EXACT_FULL_C03")
            self.assertEqual(projection["map_descriptor"]["content_hash"], bootstrap["map_manifest"]["content_hash"])
            self.assertFalse(projection["resources_are_complete_manifests"])
            self.assertNotIn("participant", projection)
            self.assertNotIn("avatar", projection)
            self.assertFalse(metadata["full_body_retained_or_hashed"])
            self.assertNotEqual(path.read_bytes(), raw)
            self.assertNotIn(principal[0], path.read_text())
            self.assertIsNone(re.search(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", path.read_text()))
            self.assertNotIn(principal[2], path.read_text())
            self.assertNotIn(principal[0], json.dumps(metadata))
        bootstrap["capabilities"]["world"] = {"available": False, "reason": "FEATURE_DISABLED"}
        with patch.object(live, "public_json", return_value=json.dumps(bootstrap).encode()), patch.object(live.dev.runtime, "private_file") as persist:
            with self.assertRaises(Failure): live.retain_public_projection(None, {}, bootstrap["event"]["event_id"], principal, Path("/not-written"))
        persist.assert_not_called()


class StagedAdmissionTests(unittest.IsolatedAsyncioTestCase):
    async def test_preissue_ticket_before_last_slot_then_switch_full(self):
        config = configuration()
        config["rooms"][0]["capacity"] = config["rooms"][1]["capacity"] = 1
        config["principals"][1]["room_index"] = 1
        contracts = Contracts()
        network = FixtureNetwork(config, contracts)
        original = network.http
        seen = []
        async def http(path, token, payload=None):
            if payload and token == config["principals"][0]["bearer"] and payload["room_id"] == config["rooms"][1]["room_id"]:
                seen.append(sum(s.room == payload["room_id"] and not s.closed for s in network.sockets))
            return await original(path, token, payload)
        network.http = http
        runner = Runner(config, False, network, contracts)
        runner.capacity = True  # Two-client control-order fixture, not a 200-socket test.
        try:
            await runner.prepare()
            self.assertEqual(seen, [0])
            await runner.switch(runner.clients[0], 1, True, auth=runner.full_room_auth)
            self.assertEqual(runner.clients[0].state.rejection, "ROOM_FULL")
        finally:
            for client in runner.clients: await client.stop()


if __name__ == "__main__": unittest.main()
