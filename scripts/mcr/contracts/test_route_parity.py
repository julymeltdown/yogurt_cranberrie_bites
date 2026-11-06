#!/usr/bin/env python3
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("route-parity.py")
SPEC = importlib.util.spec_from_file_location("route_parity", SCRIPT)
route_parity = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(route_parity)

REGISTRY = """
pub const OPERATIONS: &[RegisteredOperation] = &[
    RegisteredOperation { id: "createConcertBeacon", method: "POST", path: "/api/concert/events/{eventId}/beacons", binary: false },
    RegisteredOperation { id: "setConcertRally", method: "PUT", path: "/api/concert/events/{eventId}/parties/{partyId}/rally/me", binary: false },
];
"""

OPERATIONS = [
    {
        "operationId": "revokeConcertBeacon",
        "method": "delete",
        "path": "/api/concert/events/{eventId}/beacons/{beaconId}",
        "write_policy": "MEMBER",
    },
    {
        "operationId": "listConcertParties",
        "method": "get",
        "path": "/api/concert/events/{eventId}/parties",
        "write_policy": "READ",
    },
]

ROUTER = """
pub(super) fn router() -> Router<Arc<AppState>> {
    Router::new()
ROUTES
}
"""


def make_tree(root: Path, routes: str) -> None:
    (root / "backend/crates/concert-application/src/commands").mkdir(parents=True)
    (root / "backend/crates/concert-application/src/commands/registry.rs").write_text(REGISTRY)
    (root / "contracts/domain/v1").mkdir(parents=True)
    (root / "contracts/domain/v1/operations.json").write_text(json.dumps(OPERATIONS))
    http = root / "backend/services/api-gateway/src/adapters/in/http/concert_product"
    http.mkdir(parents=True)
    (http / "routes.rs").write_text(ROUTER.replace("ROUTES", routes))


class RouteParityTest(unittest.TestCase):
    def test_exact_templates_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(
                root,
                '\n'.join(
                    [
                        '        .route("/api/concert/events/:event_id/beacons", post(dispatch))',
                        '        .route("/api/concert/events/:event_id/beacons/:beacon_id", delete(dispatch))',
                        '        .route("/api/concert/events/:event_id/parties/:party_id/rally/me", put(dispatch))',
                    ]
                ),
            )
            self.assertEqual(route_parity.main(["--root", str(root)]), 0)

    def test_deliberately_missing_route_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(
                root,
                '\n'.join(
                    [
                        '        .route("/api/concert/events/:event_id/beacons", post(dispatch))',
                        '        .route("/api/concert/events/:event_id/parties/:party_id/rally/me", put(dispatch))',
                    ]
                ),
            )
            self.assertEqual(route_parity.missing_templates(root), [
                ("revokeConcertBeacon", "DELETE", "/api/concert/events/:event_id/beacons/:beacon_id")
            ])
            self.assertEqual(route_parity.main(["--root", str(root)]), 1)

    def test_wrong_method_on_existing_template_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(
                root,
                '\n'.join(
                    [
                        '        .route("/api/concert/events/:event_id/beacons", get(dispatch))',
                        '        .route("/api/concert/events/:event_id/beacons/:beacon_id", delete(dispatch))',
                        '        .route("/api/concert/events/:event_id/parties/:party_id/rally/me", put(dispatch))',
                    ]
                ),
            )
            self.assertEqual(route_parity.missing_templates(root), [
                ("createConcertBeacon", "POST", "/api/concert/events/:event_id/beacons")
            ])

    def test_chained_methods_count_for_each_verb(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(
                root,
                '\n'.join(
                    [
                        '        .route("/api/concert/events/:event_id/beacons", get(dispatch).post(dispatch))',
                        '        .route("/api/concert/events/:event_id/beacons/:beacon_id", delete(dispatch))',
                        '        .route("/api/concert/events/:event_id/parties/:party_id/rally/me", put(dispatch))',
                    ]
                ),
            )
            self.assertEqual(route_parity.missing_templates(root), [])


if __name__ == "__main__":
    unittest.main()
