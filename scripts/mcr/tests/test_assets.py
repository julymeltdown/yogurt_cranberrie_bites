"""C06 contract probes: schema rejection, reference integrity, geometry and CLI."""
from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / "validate-assets.py"
SPEC = importlib.util.spec_from_file_location("mcr_assets", SCRIPT)
assets = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(assets)


class AssetContractsTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.checks = assets.validators()
        cls.golden = assets.load_bundle(assets.FIXTURES)

    def setUp(self):
        self.docs = deepcopy(self.golden)
        self.avatar = self.docs["avatar-manifest.json"]
        self.config = self.docs["avatar-config.json"]
        self.map = self.docs["map-manifest.json"]
        self.theme = self.docs["theme-revenge.json"]

    def seal(self):
        """Update integrity fields so semantic probes cannot pass on hash failures."""
        self.avatar["content_hash"] = assets.content_hash(self.avatar)
        self.map["geometry_hash"] = assets.digest(self.map["geometry"])
        self.map["content_hash"] = assets.content_hash(self.map)
        for document in self.docs.values():
            if document["kind"] == "theme_manifest":
                document["map_content_hash"] = self.map["content_hash"]
                document["geometry_hash"] = self.map["geometry_hash"]
                document["avatar_content_hash"] = self.avatar["content_hash"]
                document["content_hash"] = assets.content_hash(document)

    def rejected(self, message, seal=True):
        if seal:
            self.seal()
        with self.assertRaisesRegex(assets.AssetError, message):
            assets.validate_bundle(self.docs, self.checks)

    def test_golden_bundle_and_explicit_image_limit(self):
        report = assets.validate_bundle(self.docs, self.checks)
        self.assertEqual(report["documents_validated"], 6)
        self.assertEqual(report["themes"], ["DANGER", "PARADE", "REVENGE"])
        self.assertEqual(report["atlas_image_state"], "NOT_PROVIDED")
        self.assertEqual(report["atlas_image_coverage"], "NOT_RUN")
        self.assertEqual(report["resource_bytes_and_hashes"], "NOT_RUN")

    def test_public_definition_names(self):
        for file, name in (("avatar-config.schema.json", "AvatarConfig"),
                           ("avatar-manifest.schema.json", "AvatarManifest"),
                           ("map-manifest.schema.json", "MapManifest"),
                           ("common.schema.json", "ThemeId")):
            schema = assets.read_json(assets.SCHEMAS / file)
            self.assertIn(name, schema["$defs"])
            self.assertEqual(schema["$schema"], "https://json-schema.org/draft/2020-12/schema")

    def test_missing_format_plugins_refuse_validation(self):
        # A dependency-minimal environment must never report a passing bundle
        # by silently ignoring the optional date-time and URI assertions.
        incomplete = assets.FormatChecker(formats=["uuid"])
        with patch.object(assets, "FormatChecker", return_value=incomplete):
            with self.assertRaisesRegex(assets.AssetError, "date-time, uri"):
                assets.validators()

    def test_reviewed_provenance_requires_real_datetime_format(self):
        for timestamp in ("not-a-date", "2026-02-30T00:00:00Z", "2026-11-03",
                          "2026-11-03T00:00:00"):
            with self.subTest(timestamp=timestamp):
                self.setUp()
                self.map["provenance"].update(review_state="REVIEWED",
                                               reviewed_by="format-fixture-review",
                                               reviewed_at=timestamp)
                self.rejected("schema")

    def test_provenance_requires_absolute_uri_syntax(self):
        for uri in ("relative/path", "https://example.invalid/a b",
                    "https://example.invalid/%zz"):
            with self.subTest(uri=uri):
                self.setUp()
                self.map["provenance"]["source_uri"] = uri
                self.rejected("schema")

    def test_uri_format_rejects_raw_whitespace_controls_and_nonascii(self):
        for suffix in ("\n", "\r\n", "\t", "\x00", "\x7f", "é"):
            with self.subTest(suffix=repr(suffix)):
                self.setUp()
                self.map["provenance"]["source_uri"] = "https://example.invalid/ref" + suffix
                self.rejected("schema")
        self.setUp()
        self.map["provenance"]["source_uri"] = "https://example.invalid/encoded%20space"
        self.seal()
        self.assertEqual(assets.validate_bundle(self.docs, self.checks)["status"],
                         "CONTRACT_READY")

    def test_datetime_format_rejects_raw_whitespace_controls_and_nonascii(self):
        for suffix in ("\n", "\r\n", "\t", "\x00", "\x7f", "é"):
            with self.subTest(suffix=repr(suffix)):
                self.setUp()
                self.map["provenance"].update(review_state="REVIEWED",
                                               reviewed_by="format-fixture-review",
                                               reviewed_at="2026-11-03T00:00:00Z" + suffix)
                self.rejected("schema")

    def test_valid_formats_do_not_add_uuid_version_or_timestamp_case_policy(self):
        for place_id in ("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                         "11111111-1111-0111-1111-111111111111"):
            with self.subTest(place_id=place_id):
                self.setUp()
                self.map["places"][0]["place_id"] = place_id
                self.map["geometry"]["interactions"][0]["place_id"] = place_id
                self.map["provenance"].update(review_state="REVIEWED",
                                               reviewed_by="format-fixture-review",
                                               reviewed_at="2026-11-03t00:00:00z",
                                               source_uri="urn:mcr:format-fixture")
                for document in self.docs.values():
                    if document["kind"] == "theme_manifest":
                        document["place_ids"] = [place_id]
                self.seal()
                self.assertEqual(assets.validate_bundle(self.docs, self.checks)["status"],
                                 "CONTRACT_READY")

    def test_wrong_anchor_and_frame_layout(self):
        mutations = (
            lambda: self.avatar["atlas"]["anchor"].update(y=59),
            lambda: self.avatar["atlas"]["items"][0]["anchor"].update(x=31),
            lambda: self.avatar["atlas"]["cell"].update(width_px=32),
            lambda: self.avatar["atlas"]["frame_layout"].update(walk_columns=[1, 2, 3]),
            lambda: self.avatar["atlas"]["frame_layout"].update(idle_column=1),
            lambda: self.avatar["atlas"]["frame_layout"].update(direction_order=["up", "left", "right", "down"]),
            lambda: self.avatar["atlas"]["items"][0]["region"].update(width_px=256),
        )
        for index, mutate in enumerate(mutations):
            with self.subTest(layout=index):
                self.setUp()
                mutate()
                self.rejected("schema")

    def test_texture_bounds_overlap_and_cell_alignment(self):
        cases = [("texture", "height_px", 64, "bounds"),
                 ("region", "y", 0, "overlapping"),
                 ("region", "x", 1, "cell aligned")]
        for target, field, value, message in cases:
            with self.subTest(target=target, field=field):
                self.setUp()
                node = (self.avatar["atlas"]["texture"] if target == "texture" else
                        self.avatar["atlas"]["items"][1]["region"])
                node[field] = value
                self.rejected(message)

    def test_unknown_item_and_wrong_layer(self):
        self.config["item_ids"]["hair"] = "unknown_item"
        self.rejected("unknown item_id")
        self.setUp()
        self.config["item_ids"]["hair"] = "eyes_fixture"
        self.rejected("wrong layer")

    def test_no_client_asset_url(self):
        self.config["item_ids"]["hair"] = "https://example.invalid/hair.png"
        self.rejected("schema")
        self.setUp()
        self.config["asset_url"] = "https://example.invalid/avatar.png"
        self.rejected("schema")

    def test_nullable_accessory_and_valid_fallback(self):
        self.config["item_ids"]["accessory"] = None
        self.assertEqual(assets.validate_bundle(self.docs, self.checks)["status"], "CONTRACT_READY")

    def test_unknown_skin_tone_preset_and_manifest_version(self):
        self.config["skin_tone_id"] = "unknown_tone"
        self.rejected("unknown skin_tone_id")
        self.setUp()
        self.avatar["fallback_preset_id"] = "unknown_preset"
        self.rejected("unknown fallback_preset_id")
        self.setUp()
        self.config["manifest_version"] = 2
        self.rejected("identity/version mismatch")
        self.setUp()
        self.avatar["presets"][0]["config"]["item_ids"]["body"] = "unknown_body"
        self.rejected("unknown item_id")

    def test_duplicate_ids_and_missing_layer(self):
        self.avatar["atlas"]["items"][1]["item_id"] = "body_fixture"
        self.rejected("duplicate item_id")
        self.setUp()
        self.avatar["atlas"]["items"].pop()
        self.rejected("missing required layer")
        self.setUp()
        self.map["places"].append(deepcopy(self.map["places"][0]))
        self.rejected("duplicate place_id")

    def test_theme_identity_and_reference_mismatches(self):
        for field, value in (("map_id", "different_map"), ("map_revision", 2),
                             ("collision_id", "different_collision"),
                             ("avatar_manifest_id", "different_avatar"),
                             ("avatar_manifest_version", 2),
                             ("place_ids", ["00000000-0000-4000-8000-000000000099"]),
                             ("interaction_ids", ["unknown_interaction"])):
            with self.subTest(field=field):
                self.setUp()
                self.theme[field] = value
                self.rejected(field + " mismatch")

    def test_unknown_theme_missing_theme_and_forbidden_geometry_override(self):
        self.theme["theme_id"] = "FOURTH_THEME"
        self.rejected("schema")
        self.setUp()
        del self.docs["theme-danger.json"]
        self.rejected("exactly REVENGE, PARADE, DANGER")
        self.setUp()
        self.theme["geometry"] = self.map["geometry"]
        self.rejected("schema")

    def test_stale_manifest_geometry_and_theme_hashes(self):
        self.avatar["provenance"]["source_revision"] = "changed"
        self.rejected("content_hash mismatch", seal=False)
        self.setUp()
        self.map["geometry_hash"] = "sha256:" + "0" * 64
        self.map["content_hash"] = assets.content_hash(self.map)
        self.rejected("geometry_hash mismatch", seal=False)
        for field in ("map_content_hash", "avatar_content_hash", "geometry_hash"):
            with self.subTest(field=field):
                self.setUp()
                self.theme[field] = "sha256:" + "0" * 64
                self.theme["content_hash"] = assets.content_hash(self.theme)
                self.rejected(field + " mismatch", seal=False)

    def test_spawn_and_interaction_must_be_walkable_and_in_bounds(self):
        for collection in ("spawns", "interactions"):
            for x, y in ((2, 2), (0, 0), (5, 1)):
                with self.subTest(collection=collection, x=x, y=y):
                    self.setUp()
                    self.map["geometry"][collection][0].update(cell_x=x, cell_y=y)
                    self.rejected("not walkable")

    def test_spawn_and_interaction_ids_resolve(self):
        self.map["geometry"]["default_spawn_id"] = "unknown_spawn"
        self.rejected("unknown default_spawn_id")
        self.setUp()
        self.map["geometry"]["interactions"][0]["place_id"] = "00000000-0000-4000-8000-000000000099"
        self.rejected("unknown interaction place_id")
        for collection, key in (("spawns", "spawn_id"), ("interactions", "interaction_id")):
            with self.subTest(collection=collection):
                self.setUp()
                self.map["geometry"][collection].append(deepcopy(self.map["geometry"][collection][0]))
                self.rejected("duplicate " + key)

    def test_collision_barrier_blocks_reachability(self):
        self.map["geometry"]["collision_grid"][2] = [1] * 5
        self.rejected("interaction unreachable from spawn")

    def test_grid_dimensions_and_unknown_tile(self):
        for layer in ("collision_grid", "terrain_layer", "overdraw_layer"):
            with self.subTest(layer=layer):
                self.setUp()
                self.map["geometry"][layer][0].pop()
                self.rejected("dimensions differ")
        self.setUp()
        self.map["geometry"]["terrain_layer"][0][0] = 2
        self.rejected("tile ID exceeds")

    def test_unreviewed_and_virtual_places_cannot_claim_verified(self):
        place = self.map["places"][0]
        place["verification_state"] = "VERIFIED"
        self.rejected("schema")
        self.setUp()
        place = self.map["places"][0]
        place.update(place_kind="PHYSICAL", verification_state="VERIFIED")
        self.rejected("schema")
        self.setUp()
        place = self.map["places"][0]
        place["provenance"]["review_state"] = "REVIEWED"
        self.rejected("schema")

    def test_texture_presence_needs_metadata_without_claiming_pixel_coverage(self):
        texture = self.avatar["atlas"]["texture"]
        texture["image_state"] = "PROVIDED"
        self.rejected("schema")
        texture.update(path="avatars/fixture.png", sha256="sha256:" + "0" * 64)
        self.seal()
        report = assets.validate_bundle(self.docs, self.checks)
        self.assertEqual(report["atlas_image_coverage"], "NOT_RUN")
        self.assertEqual(report["resource_bytes_and_hashes"], "NOT_RUN")

    def test_unsupported_contract_and_unknown_fields(self):
        self.config["contract_version"] = "2.0.0"
        self.rejected("schema")
        self.setUp()
        self.avatar["layer_order"] = list(reversed(self.avatar["layer_order"]))
        self.rejected("schema")

    def test_canonical_hash_and_rejection_of_ambiguous_json(self):
        self.assertEqual(assets.digest({"b": 2, "a": 1}), assets.digest({"a": 1, "b": 2}))
        self.assertEqual(assets.canonical_bytes({"label": "é"}), b'{"label":"\\u00e9"}')
        self.config["manifest_version"] = 1.0
        self.rejected("floats", seal=False)
        with tempfile.TemporaryDirectory(prefix="mcr-assets-") as directory:
            file = Path(directory) / "duplicate.json"
            file.write_text('{"kind":"avatar_config","kind":"avatar_manifest"}')
            with self.assertRaisesRegex(assets.AssetError, "duplicate JSON key"):
                assets.read_json(file)

    def test_cli_success_and_nonempty_fail_closed(self):
        result = subprocess.run([sys.executable, str(SCRIPT)], text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(json.loads(result.stdout)["atlas_image_coverage"], "NOT_RUN")
        with tempfile.TemporaryDirectory(prefix="mcr-assets-") as directory:
            command = [sys.executable, str(SCRIPT), "--fixtures", directory]
            result = subprocess.run(command, text=True, capture_output=True)
            self.assertEqual(result.returncode, 1)
            self.assertIn("empty asset bundle", result.stderr)
            file = Path(directory) / "invalid.json"
            for kind in ("unexpected_asset", [], None):
                file.write_text(json.dumps({"kind": kind}))
                result = subprocess.run(command, text=True, capture_output=True)
                self.assertEqual(result.returncode, 1)
                self.assertIn("unknown document kind", result.stderr)
            file.write_text('{"kind":')
            result = subprocess.run(command, text=True, capture_output=True)
            self.assertEqual(result.returncode, 1)
            self.assertIn("ASSETS FAIL", result.stderr)


if __name__ == "__main__":
    unittest.main()
