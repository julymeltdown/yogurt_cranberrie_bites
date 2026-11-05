#!/usr/bin/env python3
"""Synthetic source-semantic regressions; never ART or World admission evidence."""
from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
BASE = "c7c42364c5fe5401393683d845de883c0625d1ac"


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


binding = module("photo2_binding_under_test", Path(__file__).with_name("build_photo2_parade_binding.py"))


def historical(path):
    raw = subprocess.run(["git", "-C", str(ROOT), "show", BASE + ":" + path],
                         check=True, capture_output=True).stdout
    return json.loads(raw)


class BindingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.legacy = historical(binding.LEGACY + "/theme-parade.json")
        cls.bootstrap = historical(binding.LEGACY + "/bootstrap.json")
        cls.map = historical(binding.MAP)
        cls.avatar = historical("contracts/fixtures/assets/selected-static/avatar-manifest.json")
        cls.assets = module("c06_under_test", ROOT / "scripts/mcr/validate-assets.py")

    def fixture(self):
        scenery = deepcopy(self.legacy["scenery"])
        for resource, name, size in [(scenery["ground"], "ground", (499, 1200)),
                                     (scenery["atlases"][0], "atlas", (512, 1024))]:
            resource.update(id="synthetic_photo2_" + name, path=binding.PREFIX + name + ".png",
                            sha256="sha256:" + "1" * 64, bytes=1, width_px=size[0], height_px=size[1])
        frames = []
        for obj in scenery["objects"]:
            obj.update(atlas_id=scenery["atlases"][0]["id"], frame={"x": 0, "y": 0, "width": 1, "height": 1})
            frames.append({"id": obj["id"], "native": {k: obj[k] for k in ("id", "x", "y", "width", "height", "depth_y")},
                           "atlasRect": deepcopy(obj["frame"])})
        interface = {"schemaVersion": "mcr-photo2-world-art-source-v1", "version": 1,
            "themeId": "PARADE", "variantId": "photo2-parade-display-v1",
            "sourceInput": {"contractSHA256": binding.CONTRACT_SHA,
                            "nativeSource": {"sha256": binding.NATIVE_SHA, "geometryHash": binding.GEOMETRY}},
            "scenery": scenery, "sceneDescriptor": {"schema": binding.PROFILE, "version": 1,
                "nativeTileTexture": "SEPARATE_EXISTING_PINNED_C06_TILED_TEXTURE_UNCHANGED", "frames": frames,
                "boards": [{"id": "photo2-shortcut-" + name, "destination": "/" + name,
                            "kind": "nonphysical-panel-shortcut", "interactionId": None} for name in ("meet", "wall")]}}
        registration = {"art_commit": "2" * 40, "interface": {"path": "frontend/public/" + binding.PREFIX + "asset-interface.json",
            "sha256": "sha256:" + "3" * 64}, "author": "Synthetic test author", "license": "Synthetic fixture only", "reviewed_by": "SYNTHETIC SOURCE TEST ONLY", "reviewed_at": "2026-11-05T00:00:00Z"}
        return interface, registration

    def test_candidate_preserves_native_identity_and_changes_only_parade_skin(self):
        interface, registration = self.fixture()
        before = deepcopy(self.legacy)
        first = binding.make_theme(self.legacy, interface, registration, 2)
        second = binding.make_theme(self.legacy, interface, registration, 2)
        self.assertEqual(first, second)
        self.assertEqual(self.legacy, before)
        for key in set(before) - {"theme_revision", "content_hash", "scenery", "provenance"}:
            self.assertEqual(first[key], before[key], key)
        hashed = {k: v for k, v in first.items() if k != "content_hash"}
        self.assertEqual(first["content_hash"], binding.digest(binding.canonical(hashed)))

    def test_native_or_shortcut_or_frame_mutations_fail(self):
        for case in ("foot", "order", "frame", "geometry", "physical-board", "revision", "ground", "transfer"):
            with self.subTest(case=case):
                interface, registration = self.fixture()
                revision = 2
                if case == "foot": interface["scenery"]["objects"][0]["depth_y"] += 1
                if case == "order": interface["scenery"]["objects"].reverse()
                if case == "frame": interface["sceneDescriptor"]["frames"][0]["atlasRect"]["width"] += 1
                if case == "geometry": interface["sourceInput"]["nativeSource"]["geometryHash"] = "sha256:" + "0" * 64
                if case == "physical-board": interface["sceneDescriptor"]["boards"][0]["interactionId"] = "invented-arrival"
                if case == "revision": revision = 1
                if case == "ground": interface["scenery"]["ground"]["width_px"] = 768
                if case == "transfer": interface["scenery"]["ground"]["bytes"] = 2097152
                with self.assertRaises(binding.BindingError):
                    binding.make_theme(self.legacy, interface, registration, revision)

    def test_unregistered_receipt_cannot_create_release_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(binding.BindingError, "independently qualified"):
                binding.build(root, root / "absent.json", "sha256:" + "0" * 64, 2)
            self.assertEqual(list(root.iterdir()), [])

    def test_exact_file_pins_traversal_and_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "resource").write_bytes(b"abc")
            pin = binding.pin(root, "resource")
            self.assertEqual(binding.read_pin(root, pin), b"abc")
            for wrong in (dict(pin, bytes=4), dict(pin, sha256="sha256:" + "0" * 64), dict(pin, path="../resource")):
                with self.assertRaises(binding.BindingError): binding.read_pin(root, wrong)
            (root / "alias").symlink_to(root / "resource")
            with self.assertRaises(binding.BindingError): binding.read_pin(root, dict(pin, path="alias"))

    def test_duplicate_keys_and_float_c06_are_rejected(self):
        with self.assertRaises(binding.BindingError): binding.parse(b'{"x":1,"x":2}')
        with self.assertRaises(binding.BindingError): binding.canonical({"x": 0.5})

    def documents(self, parade=None):
        return {"map": self.map, "avatar": self.avatar, "config": self.bootstrap["avatar"]["config"],
                **{t["theme_id"]: parade if t["theme_id"] == "PARADE" and parade else t
                   for t in self.bootstrap["theme_manifests"]}}

    def test_full_c06_legacy_bundle_still_validates(self):
        result = self.assets.validate_bundle(self.documents())
        self.assertEqual(result["status"], "CONTRACT_READY")

    def test_c06_registered_source_fixture_and_unregistered_namespace(self):
        interface, registration = self.fixture()
        candidate = binding.make_theme(self.legacy, interface, registration, 2)
        with self.assertRaisesRegex(self.assets.AssetError, "unregistered Photo2"):
            self.assets.validate_bundle(self.documents(candidate))
        # Process-local synthetic registration exercises cross-field semantics only.
        # Synthetic fixture is isolated from the production literal tuple.
        local = {"theme_hash": candidate["content_hash"], "scenery_hash": binding.digest(binding.canonical(candidate["scenery"]))}
        with patch.object(self.assets, "PHOTO2_REGISTERED_THEMES", (local,)):
            self.assertEqual(self.assets.validate_bundle(self.documents(candidate))["status"], "CONTRACT_READY")
            candidate["texture"]["height_px"] = 1200
            candidate.pop("content_hash")
            candidate["content_hash"] = binding.digest(binding.canonical(candidate))
            with self.assertRaises(self.assets.AssetError): self.assets.validate_bundle(self.documents(candidate))

    def test_legacy_ground_extent_is_not_relaxed(self):
        scenery = deepcopy(self.legacy["scenery"])
        scenery["ground"]["height_px"] = 1200
        with self.assertRaisesRegex(self.assets.AssetError, "ground dimensions"):
            self.assets.validate_scenery(scenery, self.map["geometry"])


if __name__ == "__main__":
    unittest.main()
