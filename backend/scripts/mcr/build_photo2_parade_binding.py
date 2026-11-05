#!/usr/bin/env python3
"""Deterministic backend Photo2 binding; no ART generation or unqualified activation.

The literal qualification table binds an immutable source-qualified v3 tuple.
Candidate derivation is pure; CLI publication additionally
requires registered C03 theme/scenery pins and complete schema/bundle validation.
"""
import argparse
from copy import deepcopy
import hashlib
import importlib.util
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[3]
PREFIX = "world/reference-scene-v2/photo2-world-display-v1/parade/"
PROFILE = "mcr-photo2-world-display-v1"
OUT = "backend/releases/photo2-world-display-v1"
LEGACY = "backend/releases/operator-bundle/quality-v6"
MAP = "backend/releases/reference-scene-v2/quality-v6/map-manifest.json"
BINDING = "backend/releases/reference-scene-v2/quality-v6/source-binding.json"
CONTRACT_SHA = "cc12d028b4615246ea457722d13af6d35fc505e44b6d1946adcf1811a37ab98e"
NATIVE_SHA = "847d9c92a4ddfa22b8187bb6afe9a9fc2e6e09b1c7fe8e5f85c5a7f5c0d846a9"
GEOMETRY = "sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f"
NATIVE_OBJECTS = "sha256:8305532e5438901164b2fcd208b9603ea32d2779a5b6cbaacb277685e4b05499"
# key: exact genuine qualifier receipt SHA; value: independent literal producer
# commit, interface/provenance file pins, reviewed_by and reviewed_at.
# Not populated from receipt assertions, command-line input or synthetic tests.
REGISTERED_ART_TUPLES = {'sha256:7ef35cbbbff44ae3fe0a0681f984f611fb1d9f744d2432145ff0ec245164894f': {'art_commit': 'a8027217ec9bc5b33a9219b74726ea707f33e0cc', 'producer_commit': 'a8027217ec9bc5b33a9219b74726ea707f33e0cc', 'interface': {'path': 'frontend/public/world/reference-scene-v2/photo2-world-display-v1/parade/asset-interface.json', 'bytes': 46552, 'sha256': 'sha256:dd2d09f2aebe3c631fcd0eed8cc09e11df8661550a38fa214ee50e6c39b81600'}, 'provenance': {'path': 'frontend/public/world/reference-scene-v2/photo2-world-display-v1/parade/source-provenance.json', 'bytes': 3469, 'sha256': 'sha256:8cebea0e8f5adcb9fa479b01936e89e65fa15a1a625f5afc47bd0a898e9ed465'}, 'reviewed_by': 'MCR ART producer 01a1a231-0dc4-7b50-92bd-c0f0f56bd970', 'reviewed_at': '2026-11-05T08:16:47.835483Z', 'author': 'MCR ART producer; original editable vector primitives and deterministic rasterization', 'license': 'Original project-authored artwork; no external license asserted', 'source_qualification': 'SOURCE_QUALIFIED_PERSONALLY_INSPECTED_LAMP_FIX_COMPLETE', 'active_fix': 'World renderer consumption and full Photo2/native/runtime acceptance remain open; lamp source correction complete', 'world_runtime_acceptance': False, 'full_photo2_acceptance': False, 'source_kind': 'FILESYSTEM_MANIFEST_PINNED_SOURCE', 'lamp_fix_source_complete': True}}


class BindingError(ValueError):
    pass


def require(ok, message):
    if not ok:
        raise BindingError(message)


def digest(raw):
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical(value):
    def integers(node):
        require(type(node) in (dict, list, str, int, bool, type(None)), "C06 integer JSON required")
        if isinstance(node, dict):
            for key, item in node.items():
                require(isinstance(key, str), "non-string JSON key")
                integers(item)
        elif isinstance(node, list):
            for item in node:
                integers(item)
    integers(value)
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()


def parse(raw):
    def pairs(items):
        out = {}
        for key, value in items:
            require(key not in out, "duplicate JSON key")
            out[key] = value
        return out
    return json.loads(raw, object_pairs_hook=pairs,
                      parse_constant=lambda _: (_ for _ in ()).throw(BindingError("nonfinite JSON")))


def path_under(root, relative):
    require(isinstance(relative, str) and re.fullmatch(r"[A-Za-z0-9_/.-]{1,512}", relative), "invalid path")
    require(all(part not in ("", ".", "..") for part in relative.split("/")), "unsafe path")
    root = root.resolve()
    path = root
    for part in relative.split("/"):
        path = path / part
        require(not path.is_symlink(), "symlink resource")
    require(path.resolve().is_relative_to(root), "root escape")
    return path


def read_pin(root, pin):
    path = path_under(root, pin["path"])
    raw = path.read_bytes()
    require(0 < len(raw) <= 8 * 1024 * 1024 and len(raw) == pin["bytes"]
            and digest(raw) == pin["sha256"], "file bytes/hash mismatch")
    return raw


def pin(root, relative):
    raw = path_under(root, relative).read_bytes()
    return {"path": relative, "bytes": len(raw), "sha256": digest(raw)}


def public_pin(value):
    require(value["path"].startswith("frontend/public/"), "not a public asset pin")
    return dict(value, path=value["path"].removeprefix("frontend/public/"))


def qualification(root, receipt, expected_sha):
    require(expected_sha in REGISTERED_ART_TUPLES, "ART tuple not independently qualified/registered")
    raw = receipt.read_bytes()
    require(digest(raw) == expected_sha, "qualifier receipt hash mismatch")
    parse(raw)  # Exact registered receipt bytes are authoritative; no invented ART receipt schema.
    registration = REGISTERED_ART_TUPLES[expected_sha]
    require(re.fullmatch(r"[0-9a-f]{40}", registration["art_commit"]), "invalid ART commit")
    interface = parse(read_pin(root, registration["interface"]))
    provenance = read_pin(root, registration["provenance"])
    parse(provenance)
    require(registration["interface"]["path"] == "frontend/public/" + PREFIX + "asset-interface.json"
            and registration["provenance"]["path"] == "frontend/public/" + PREFIX + "source-provenance.json",
            "qualified namespace mismatch")
    return registration, interface


def make_theme(legacy, interface, registration, revision):
    """Pure candidate derivation. Calling this does not register/admit a release."""
    require(type(revision) is int and revision >= 2, "PARADE revision must be >=2")
    require(legacy["theme_id"] == "PARADE" and legacy["map_revision"] == 4
            and legacy["geometry_hash"] == GEOMETRY, "wrong legacy/native release")
    require(interface["schemaVersion"] == "mcr-photo2-world-art-source-v1"
            and interface["version"] == 1 and interface["themeId"] == "PARADE"
            and interface["variantId"] == "photo2-parade-display-v1", "wrong ART interface")
    source = interface["sourceInput"]
    require(source["contractSHA256"] == CONTRACT_SHA
            and source["nativeSource"]["sha256"] == NATIVE_SHA
            and source["nativeSource"]["geometryHash"] == GEOMETRY, "mixed source contract")
    scenery, scene = interface["scenery"], interface["sceneDescriptor"]
    require(scene["schema"] == PROFILE and scene["version"] == 1
            and scene["nativeTileTexture"] == "SEPARATE_EXISTING_PINNED_C06_TILED_TEXTURE_UNCHANGED",
            "native/display descriptor mismatch")
    require(scenery["version"] == 1 and len(scenery["atlases"]) == 1, "wrong scenery version/atlas count")
    textures = [scenery["ground"], scenery["atlases"][0]]
    for texture, name, dimensions in zip(textures, ("ground", "atlas"), ((499, 1200), (512, 1024))):
        require(texture["path"] == PREFIX + name + ".png"
                and (texture["width_px"], texture["height_px"]) == dimensions
                and type(texture["bytes"]) is int and texture["bytes"] > 0
                and re.fullmatch(r"sha256:[0-9a-f]{64}", texture["sha256"]), "wrong display resource")
    require(sum(t["bytes"] for t in textures) <= 2097152, "scenery transfer budget exceeded")
    native = lambda objects: [{k: v for k, v in obj.items() if k not in ("atlas_id", "frame")} for obj in objects]
    objects = scenery["objects"]
    require(len(objects) == 22 and native(objects) == native(legacy["scenery"]["objects"])
            and digest(canonical(native(objects))) == NATIVE_OBJECTS, "native objects/order/feet changed")
    frames = scene["frames"]
    require(len(frames) == 22, "wrong landmark count")
    for frame, obj in zip(frames, objects):
        require(frame["id"] == obj["id"] and frame["atlasRect"] == obj["frame"]
                and all(frame["native"][k] == obj[k] for k in ("id", "x", "y", "width", "height", "depth_y"))
                and obj["atlas_id"] == textures[1]["id"], "mixed native/frame tuple")
    require(len(scene["boards"]) == 2, "wrong panel shortcut count")
    for board, identity, destination in zip(scene["boards"],
            ("photo2-shortcut-meet", "photo2-shortcut-wall"), ("/meet", "/wall")):
        require(board["id"] == identity and board["destination"] == destination
                and board["kind"] == "nonphysical-panel-shortcut" and "interactionId" in board
                and board["interactionId"] is None, "physical or unknown shortcut")
    result = deepcopy(legacy)
    result.update(theme_revision=revision, scenery=deepcopy(scenery))
    result["provenance"] = dict(legacy["provenance"], source_id="mcr_photo2_parade_display_v1",
        source_kind="ORIGINAL_ART", author=registration["author"], license=registration["license"],
        photo_permission="NOT_APPLICABLE",
        source_revision=registration["art_commit"] + "; interface " + registration["interface"]["sha256"],
        source_uri="https://github.com/julymeltdown/mcr/blob/" + registration["art_commit"] + "/" + registration["interface"]["path"],
        reviewed_by=registration["reviewed_by"], reviewed_at=registration["reviewed_at"], review_state="REVIEWED")
    result.pop("content_hash")
    result["content_hash"] = digest(canonical(result))
    return result


def build(root, receipt, expected_sha, revision):
    registration, interface = qualification(root, receipt, expected_sha)
    read = lambda relative: parse(path_under(root, relative).read_bytes())
    old_bundle = read(LEGACY + "/bundle.json")
    # Immutable legacy inputs, not merely whichever files happen to be present.
    for value in [old_bundle["map"], old_bundle["avatar"], old_bundle["source_binding"], *old_bundle["outputs"]]:
        read_pin(root, value)
    old_theme = read(LEGACY + "/theme-parade.json")
    theme = make_theme(old_theme, interface, registration, revision)
    for texture in [theme["scenery"]["ground"], *theme["scenery"]["atlases"]]:
        raw = read_pin(root, dict(texture, path="frontend/public/" + texture["path"]))
        require(raw[:8] == b"\x89PNG\r\n\x1a\n" and raw[12:16] == b"IHDR" and len(raw) >= 33,
                "invalid PNG header")
        require((int.from_bytes(raw[16:20], "big"), int.from_bytes(raw[20:24], "big"))
                == (texture["width_px"], texture["height_px"]), "PNG dimensions mismatch")
    bootstrap = read(LEGACY + "/bootstrap.json")
    bootstrap["theme_manifests"] = [theme if t["theme_id"] == "PARADE" else t for t in bootstrap["theme_manifests"]]
    source_binding = read(BINDING)
    source_binding.update(version=3, art_commit=registration["art_commit"])
    for scene in source_binding["scenes"]:
        if scene["theme_id"] == "PARADE":
            scene.update(display_profile=PROFILE, scenery_source=public_pin(registration["interface"]),
                scene_sources=[public_pin(registration["provenance"])],
                runtime_resources=[{k: v for k, v in texture.items() if k != "id"}
                    for texture in [theme["scenery"]["ground"], *theme["scenery"]["atlases"]]])
    # This shared gate independently requires literal theme/scenery registration.
    spec = importlib.util.spec_from_file_location("mcr_c06_assets", root / "scripts/mcr/validate-assets.py")
    assets = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(assets)
    documents = {"map": read(MAP), "avatar": read(old_bundle["avatar"]["path"]),
                 "config": bootstrap["avatar"]["config"],
                 **{t["theme_id"]: t for t in bootstrap["theme_manifests"]}}
    assets.validate_bundle(documents)
    inputs = {"format": "MCR_PHOTO2_QUALIFIED_INPUTS", "version": 1,
              "qualification_sha256": expected_sha, "art_commit": registration["art_commit"],
              "interface": registration["interface"], "provenance": registration["provenance"],
              "theme_hash": theme["content_hash"], "scenery_hash": digest(canonical(theme["scenery"])),
              "producer_commit": registration["producer_commit"],
              "source_qualification": registration["source_qualification"],
              "active_fix": registration["active_fix"], "world_runtime_acceptance": False,
              "full_photo2_acceptance": False, "source_kind": registration["source_kind"],
              "lamp_fix_source_complete": registration["lamp_fix_source_complete"]}
    outputs = {OUT + "/theme-parade.json": theme, OUT + "/bootstrap.json": bootstrap,
               OUT + "/source-binding.json": source_binding, OUT + "/qualified-inputs.json": inputs}
    encoded = {p: (json.dumps(v, sort_keys=True, indent=2, ensure_ascii=True) + "\n").encode() for p, v in outputs.items()}
    bundle = deepcopy(old_bundle)
    bundle.update(format="MCR_PHOTO2_OPERATOR_BUNDLE_CANDIDATE", version=1,
        source_binding={"path": OUT + "/source-binding.json", "bytes": len(encoded[OUT + "/source-binding.json"]), "sha256": digest(encoded[OUT + "/source-binding.json"])},
        qualification={"path": str(receipt.resolve().relative_to(root.resolve())), "bytes": receipt.stat().st_size, "sha256": expected_sha},
        qualified_inputs={"path": OUT + "/qualified-inputs.json", "bytes": len(encoded[OUT + "/qualified-inputs.json"]), "sha256": digest(encoded[OUT + "/qualified-inputs.json"])},
        generator=pin(root, "backend/scripts/mcr/build_photo2_parade_binding.py"),
        outputs=[{"path": p, "bytes": len(raw), "sha256": digest(raw)} for p, raw in encoded.items()]
            + [pin(root, LEGACY + "/theme-" + era + ".json") for era in ("revenge", "danger")])
    bundle["source_qualification"] = bundle["qualification"]
    bundle["limits"] += [registration["source_qualification"], registration["active_fix"],
        "World runtime and full Photo2 visual acceptance remain open; candidate is not admitted."]
    encoded[OUT + "/bundle.json"] = (json.dumps(bundle, sort_keys=True, indent=2, ensure_ascii=True) + "\n").encode()
    return encoded


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qualification", type=Path, required=True)
    parser.add_argument("--qualification-sha256", required=True)
    parser.add_argument("--theme-revision", type=int, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    encoded = build(ROOT, args.qualification, args.qualification_sha256, args.theme_revision)
    for relative, raw in encoded.items():
        destination = path_under(ROOT, relative)
        if args.check:
            require(destination.read_bytes() == raw, "generated release drift: " + relative)
        else:
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(raw)
    print("PHOTO2_BOUND_CANDIDATE_NOT_RUNTIME_ADMITTED")


if __name__ == "__main__":
    main()
