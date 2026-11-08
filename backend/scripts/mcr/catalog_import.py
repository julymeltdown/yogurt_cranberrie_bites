#!/usr/bin/env python3
"""WP-03 T-801: seed/import path for the favorites catalog.

The destination had no music/anime/game catalog. Rather than building a
provider-integration system, the catalog is one table (`catalog_items`,
migration `0016`) plus this importer. The importer reads a closed JSON manifest
and writes rows, so the catalog is reviewable in a diff instead of arriving from
a network call.

Two rules are enforced here *and* in the schema, deliberately:

* a music row must carry an explicit exclusion review, and
* a K-pop row cannot be stored in the music category at all.

The importer refuses an excluded item before it writes, so the operator sees a
clear message; the database constraint
(`catalog_items_music_excludes_kpop`) is what makes the rule true for every
writer, including one that bypasses this script.

Usage:
    python3 backend/scripts/mcr/catalog_import.py --database-url URL [--manifest PATH]
    python3 backend/scripts/mcr/catalog_import.py --database-url URL --dry-run
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys
import uuid

ROOT = Path(__file__).resolve().parents[3]
DEFAULT_MANIFEST = ROOT / "backend/services/profile-service/catalog/catalog-seed.v1.json"
CATEGORIES = ("music", "anime", "game")
FORMAT = "mcr.catalog.v1"
LABEL_MAX = 80
URL = re.compile(r"^https://[^\s]+$")


class ImportError_(Exception):
    pass


def load(path: Path) -> list[dict]:
    document = json.loads(path.read_text())
    if document.get("format") != FORMAT:
        raise ImportError_(f"manifest format must be {FORMAT}")
    items = document.get("items")
    if not isinstance(items, list) or not items:
        raise ImportError_("manifest requires a non-empty items array")
    seen: set[tuple[str, str]] = set()
    rows: list[dict] = []
    for index, item in enumerate(items):
        if not isinstance(item, dict):
            raise ImportError_(f"item {index} is not an object")
        unknown = set(item) - {"category", "label", "external_url", "excluded", "exclusion_reason"}
        if unknown:
            raise ImportError_(f"item {index} has unknown fields {sorted(unknown)}")
        category = item.get("category")
        if category not in CATEGORIES:
            raise ImportError_(f"item {index} category must be one of {CATEGORIES}")
        label = item.get("label")
        if not isinstance(label, str) or not 1 <= len(label.strip()) <= LABEL_MAX:
            raise ImportError_(f"item {index} label must be 1..{LABEL_MAX} characters")
        if re.search(r"[\x00-\x1f\x7f]", label):
            raise ImportError_(f"item {index} label contains a control character")
        url = item.get("external_url")
        if url is not None and (not isinstance(url, str) or not URL.match(url)):
            raise ImportError_(f"item {index} external_url must be https")
        excluded = item.get("excluded", False)
        if not isinstance(excluded, bool):
            raise ImportError_(f"item {index} excluded must be a boolean")
        reason = item.get("exclusion_reason")
        if reason is not None and (not isinstance(reason, str) or not 1 <= len(reason) <= 120):
            raise ImportError_(f"item {index} exclusion_reason must be 1..120 characters")
        if excluded and category != "music":
            raise ImportError_(f"item {index} may only exclude a music row")
        if excluded and reason is None:
            raise ImportError_(f"item {index} excluded row requires exclusion_reason")
        if category == "music" and reason is None:
            raise ImportError_(f"item {index} music row requires an exclusion review")
        key = (category, label.strip().lower())
        if key in seen:
            raise ImportError_(f"item {index} duplicates {key}")
        seen.add(key)
        rows.append({
            "catalog_item_id": str(uuid.uuid5(uuid.NAMESPACE_URL, f"mcr-catalog-v1:{category}:{label.strip().lower()}")),
            "category": category,
            "label": label.strip(),
            "external_url": url,
            "excluded": excluded,
            "exclusion_reason": reason,
        })
    return rows


def store(rows: list[dict], database_url: str) -> dict:
    """Insert the storable rows. Excluded rows are policy records, never stored.

    The write goes through `psql` rather than a Python driver because the
    repository's Python environment has no PostgreSQL driver and the private
    cluster harness already supplies `psql`. The statements are generated here
    from validated values, never from raw manifest text.
    """
    import subprocess

    storable = [row for row in rows if not row["excluded"]]
    statements = []
    for row in storable:
        def literal(value):
            if value is None:
                return "null"
            return "'" + str(value).replace("'", "''") + "'"
        statements.append(
            "insert into catalog_items(catalog_item_id,category,label,external_url,"
            "source,excluded,exclusion_reason) values("
            f"'{row['catalog_item_id']}',{literal(row['category'])},{literal(row['label'])},"
            f"{literal(row['external_url'])},'seed',false,{literal(row['exclusion_reason'])}) "
            "on conflict (category, lower(label)) do update "
            "set label=excluded.label, external_url=excluded.external_url, "
            "exclusion_reason=excluded.exclusion_reason, status='ACTIVE';"
        )
    script = "begin;\n" + "\n".join(statements) + "\ncommit;\n"
    result = subprocess.run(
        ["psql", "-X", "--no-password", "-v", "ON_ERROR_STOP=1", "--quiet",
         "--dbname", database_url, "--file", "-"],
        input=script, capture_output=True, text=True, timeout=120,
    )
    if result.returncode != 0:
        raise ImportError_(f"catalog import failed: {result.stderr.strip()[-600:]}")
    counts = subprocess.run(
        ["psql", "-X", "--no-password", "-v", "ON_ERROR_STOP=1", "-A", "-t",
         "--dbname", database_url, "-c",
         "select category || '=' || count(*) from catalog_items "
         "where status='ACTIVE' group by category order by category"],
        capture_output=True, text=True, timeout=60,
    )
    if counts.returncode != 0:
        raise ImportError_(f"catalog count failed: {counts.stderr.strip()[-600:]}")
    return {
        "stored": len(storable),
        "excluded": len(rows) - len(storable),
        "counts": dict(line.split("=", 1) for line in counts.stdout.split() if "=" in line),
    }


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--database-url", required=True)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)
    try:
        rows = load(args.manifest)
    except ImportError_ as error:
        print(f"catalog manifest rejected: {error}", file=sys.stderr)
        return 2
    if args.dry_run:
        print(json.dumps({"rows": len(rows),
                          "excluded": sum(1 for row in rows if row["excluded"])}, sort_keys=True))
        return 0
    try:
        result = store(rows, args.database_url)
    except ImportError_ as error:
        print(str(error), file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
