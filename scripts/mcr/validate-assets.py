#!/usr/bin/env python3
"""Offline C06 schema and bundle validation. Does not certify artwork or venues."""
import argparse
from collections import deque
import hashlib
import json
from pathlib import Path
import sys

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource
from referencing.exceptions import NoSuchResource

ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "contracts/assets/v1"
FIXTURES = ROOT / "contracts/fixtures/assets"
BASE = "https://contracts.mcr.invalid/assets/v1/"
KINDS = {
    "avatar_config": "avatar-config.schema.json",
    "avatar_manifest": "avatar-manifest.schema.json",
    "map_manifest": "map-manifest.schema.json",
    "theme_manifest": "theme-manifest.schema.json",
}
LAYERS = ("body", "eyes", "hair", "outfit", "accessory")
THEMES = {"REVENGE", "PARADE", "DANGER"}


class AssetError(ValueError):
    """An invalid asset contract or inconsistent bundle."""


def require(condition, message):
    if not condition:
        raise AssetError(message)


def canonical_bytes(value):
    """C06 canonical JSON: sorted keys, ASCII escaping, no whitespace or floats."""
    def inspect(node):
        require(type(node) in (dict, list, str, int, bool, type(None)),
                "canonical JSON permits integers only; floats are forbidden")
        if isinstance(node, dict):
            require(all(isinstance(key, str) for key in node), "JSON keys must be strings")
            for child in node.values():
                inspect(child)
        elif isinstance(node, list):
            for child in node:
                inspect(child)
    inspect(value)
    return json.dumps(value, ensure_ascii=True, sort_keys=True,
                      separators=(",", ":"), allow_nan=False).encode("ascii")


def digest(value):
    return "sha256:" + hashlib.sha256(canonical_bytes(value)).hexdigest()


def content_hash(document):
    return digest({key: value for key, value in document.items() if key != "content_hash"})


def reject_duplicates(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def read_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=reject_duplicates)
    except (OSError, UnicodeError, ValueError) as error:
        raise AssetError(f"{path}: {error}") from error


def offline_only(uri):
    raise NoSuchResource(ref=uri)


def validators(schema_dir=SCHEMAS):
    schemas = [read_json(path) for path in sorted(schema_dir.glob("*.schema.json"))]
    require(len(schemas) == 5, "expected all five v1 asset schemas")
    resources = []
    for schema in schemas:
        Draft202012Validator.check_schema(schema)
        resources.append((schema["$id"], Resource.from_contents(schema)))
    require(len({uri for uri, _ in resources}) == len(resources), "duplicate schema $id")
    registry = Registry(retrieve=offline_only).with_resources(resources)
    by_id = dict((schema["$id"], schema) for schema in schemas)
    return {
        kind: Draft202012Validator(by_id[BASE + filename], registry=registry,
                                    format_checker=FormatChecker())
        for kind, filename in KINDS.items()
    }


def validate_document(document, checks, label):
    require(isinstance(document, dict), f"{label}: expected JSON object")
    canonical_bytes(document)
    kind = document.get("kind")
    require(isinstance(kind, str) and kind in checks, f"{label}: unknown document kind {kind!r}")
    errors = sorted(checks[kind].iter_errors(document), key=lambda error: str(error.json_path))
    if errors:
        error = errors[0]
        raise AssetError(f"{label}: schema {error.json_path}: {error.message}")
    if kind != "avatar_config":
        require(document["content_hash"] == content_hash(document),
                f"{label}: content_hash mismatch")


def index_unique(entries, field, label):
    result = {}
    for entry in entries:
        key = entry[field]
        require(key not in result, f"{label}: duplicate {field} {key}")
        result[key] = entry
    return result


def validate_config(config, avatar, items, tones):
    require((config["manifest_id"], config["manifest_version"]) ==
            (avatar["manifest_id"], avatar["manifest_version"]), "avatar manifest identity/version mismatch")
    for layer, item_id in config["item_ids"].items():
        if layer == "accessory" and item_id is None:
            continue
        require(item_id in items, f"unknown item_id {item_id}")
        require(items[item_id]["layer_id"] == layer, f"item {item_id} belongs to wrong layer {layer}")
    require(config["skin_tone_id"] in tones, f"unknown skin_tone_id {config['skin_tone_id']}")


def validate_avatar(avatar, configs):
    atlas = avatar["atlas"]
    items = index_unique(atlas["items"], "item_id", "atlas")
    tones = index_unique(avatar["skin_tones"], "skin_tone_id", "avatar")
    presets = index_unique(avatar["presets"], "preset_id", "avatar")
    require(avatar["fallback_preset_id"] in presets, "unknown fallback_preset_id")
    require({item["layer_id"] for item in items.values()} == set(LAYERS), "atlas missing required layer")
    texture = atlas["texture"]
    require(texture["width_px"] % 64 == texture["height_px"] % 64 == 0,
            "atlas texture dimensions must be multiples of 64")
    regions = []
    for item in items.values():
        region = item["region"]
        x, y, width, height = (region[key] for key in ("x", "y", "width_px", "height_px"))
        require(x % 64 == y % 64 == 0, f"item {item['item_id']}: region is not cell aligned")
        require(x + width <= texture["width_px"] and y + height <= texture["height_px"],
                f"item {item['item_id']}: frame layout exceeds texture bounds")
        for old_x, old_y, old_w, old_h in regions:
            require(x + width <= old_x or old_x + old_w <= x or
                    y + height <= old_y or old_y + old_h <= y, "overlapping atlas item regions")
        regions.append((x, y, width, height))
    for config in [*configs, *(preset["config"] for preset in presets.values())]:
        # Preset configs were also checked through the manifest's schema reference.
        validate_config(config, avatar, items, tones)


def validate_map(map_):
    geometry = map_["geometry"]
    require(map_["geometry_hash"] == digest(geometry), "geometry_hash mismatch")
    width, height = geometry["width_cells"], geometry["height_cells"]
    for layer in ("collision_grid", "terrain_layer", "overdraw_layer"):
        rows = geometry[layer]
        require(len(rows) == height and all(len(row) == width for row in rows),
                f"{layer}: dimensions differ from map geometry")
    places = index_unique(map_["places"], "place_id", "map")
    spawns = index_unique(geometry["spawns"], "spawn_id", "map")
    interactions = index_unique(geometry["interactions"], "interaction_id", "map")
    require(geometry["default_spawn_id"] in spawns, "unknown default_spawn_id")

    def walkable(x, y):
        return 0 <= x < width and 0 <= y < height and geometry["collision_grid"][y][x] == 0

    for place in places.values():
        if place["provenance"]["source_kind"] == "ORIGINAL_FIXTURE":
            require(place["verification_state"] == "CONCEPT_ONLY" and
                    place["place_kind"] == "VIRTUAL_CONCEPT", "fixture cannot verify a physical venue")
    for entry in [*spawns.values(), *interactions.values()]:
        require(walkable(entry["cell_x"], entry["cell_y"]),
                "spawn/interaction cell is outside map or not walkable")
    for entry in interactions.values():
        require(entry["place_id"] in places, f"unknown interaction place_id {entry['place_id']}")
    # Every advertised interaction must be reachable from every encoded spawn.
    for spawn in spawns.values():
        origin = (spawn["cell_x"], spawn["cell_y"])
        queue, visited = deque([origin]), {origin}
        while queue:
            x, y = queue.popleft()
            for neighbor in ((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)):
                if neighbor not in visited and walkable(*neighbor):
                    visited.add(neighbor)
                    queue.append(neighbor)
        require(all((entry["cell_x"], entry["cell_y"]) in visited for entry in interactions.values()),
                "interaction unreachable from spawn")
    return places, interactions


def validate_bundle(documents, checks=None):
    checks = checks or validators()
    groups = {kind: [] for kind in KINDS}
    require(bool(documents), "empty asset bundle")
    for label, document in sorted(documents.items()):
        validate_document(document, checks, label)
        groups[document["kind"]].append(document)
    for kind in ("avatar_manifest", "map_manifest", "avatar_config"):
        require(len(groups[kind]) == 1, f"bundle requires exactly one {kind}")
    avatar, map_ = groups["avatar_manifest"][0], groups["map_manifest"][0]
    validate_avatar(avatar, groups["avatar_config"])
    places, interactions = validate_map(map_)
    themes = index_unique(groups["theme_manifest"], "theme_id", "themes")
    require(set(themes) == THEMES, "bundle requires exactly REVENGE, PARADE, DANGER")
    for theme in themes.values():
        for key, expected in {
            "map_id": map_["map_id"], "map_revision": map_["map_revision"],
            "map_content_hash": map_["content_hash"], "geometry_hash": map_["geometry_hash"],
            "collision_id": map_["geometry"]["collision_id"],
            "avatar_manifest_id": avatar["manifest_id"],
            "avatar_manifest_version": avatar["manifest_version"],
            "avatar_content_hash": avatar["content_hash"],
        }.items():
            require(theme[key] == expected, f"{theme['theme_id']}: {key} mismatch")
        require(set(theme["place_ids"]) == set(places), f"{theme['theme_id']}: place_ids mismatch")
        require(set(theme["interaction_ids"]) == set(interactions),
                f"{theme['theme_id']}: interaction_ids mismatch")
        geometry, texture = map_["geometry"], theme["texture"]
        tile_w, tile_h = geometry["tile_width_px"], geometry["tile_height_px"]
        require(texture["width_px"] % tile_w == texture["height_px"] % tile_h == 0,
                "theme texture dimensions not aligned to map tiles")
        tile_count = (texture["width_px"] // tile_w) * (texture["height_px"] // tile_h)
        require(all(gid <= tile_count for layer in ("terrain_layer", "overdraw_layer")
                    for row in geometry[layer] for gid in row), "tile ID exceeds theme texture layout")
    return {
        "status": "CONTRACT_READY", "documents_validated": len(documents),
        "themes": sorted(themes), "contract_version": "1.0.0",
        "atlas_image_coverage": "NOT_RUN",
        "atlas_image_state": avatar["atlas"]["texture"]["image_state"],
        "resource_bytes_and_hashes": "NOT_RUN",
        "limits": "Metadata and geometry only. No PNG/Tiled file bytes, pixel coverage, clipping, "
                  "contrast, license truth, physical venue truth, AR03 or mobile acceptance validated. "
                  "Integrator review/freeze pending.",
    }


def load_bundle(directory):
    require(directory.is_dir(), f"asset bundle directory not found: {directory}")
    return {path.relative_to(directory).as_posix(): read_json(path)
            for path in sorted(directory.rglob("*.json"))}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures", type=Path, default=FIXTURES,
                        help="validate every JSON document in this self-contained asset bundle")
    args = parser.parse_args()
    try:
        report = validate_bundle(load_bundle(args.fixtures))
    except (AssetError, OSError, ValueError, KeyError) as error:
        print(f"ASSETS FAIL: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
