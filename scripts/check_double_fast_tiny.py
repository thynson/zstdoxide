#!/usr/bin/env python3
"""Verify frozen initial DoubleFast observations offline, without a compiler."""

from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import sys

ROOT = Path(__file__).resolve().parents[1]
COLLECTION = ROOT / "tests/upstream/double-fast-tiny"
HELPER = "scripts/oracles/double_fast_direct.c"
DRIVER = "scripts/oracles/double_fast_direct.py"
CAPTURE = "scripts/capture_double_fast_tiny.py"
CHECKER = "scripts/check_double_fast_tiny.py"
PINS = {
    HELPER: "e20d452423a3f913831ac04f6edd79be3d0d18fb0fe6609357ef4ea47816d9d4",
    DRIVER: "5280b94e6e10e34a263b7ae1b31a24026adfd92327767b776e81344362604065",
    CAPTURE: "5dea723222db1171e6302411d4d59870e01a09dfd7df74bf47c51cc6978bef19",
}
CATALOG_SHA256 = "d8d61cda4f68e56cc403505d9848d09b4dd03138357457c0b0b68a82472ec94c"
COVERAGE = {"kind": "constructed-direct-tiny", "recipes": 15, "observations": 30,
            "padding_patterns": [106, 195], "entry_point": "ZSTD_compressBlock_doubleFast",
            "collector": False, "ordinary_frames": False, "chronological_trace": False}


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_oracle():
    # Verify the recipe/validator identity BEFORE importing or executing it.
    for name, digest in PINS.items():
        require(sha(ROOT / name) == digest, "frozen tool identity: " + name)
    spec = importlib.util.spec_from_file_location("zstdoxide_double_fast_tiny_oracle", ROOT / DRIVER)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def check(collection):
    oracle = load_oracle()
    collection = collection.resolve()
    require(collection.is_dir(), "missing fixture collection")
    cases = oracle.recipes()
    fixture_names = {"recipes.json"} | {"normal-" + case.name + ".stdout" for case in cases}
    actual = set()
    for path in collection.rglob("*"):
        require(not path.is_symlink(), "fixture symlink is not permitted")
        require(path.is_file(), "unexpected collection subdirectory")
        actual.add(path.relative_to(collection).as_posix())
    require(actual == fixture_names | {"manifest.json", "README.md", "NOTICE"}, "exact collection paths")

    def read_json(name):
        path = collection / name
        require(path.stat().st_size <= 2 << 20, "metadata byte cap")
        return oracle.decode_json(path.read_bytes())

    manifest = read_json("manifest.json")
    require(type(manifest) is dict and oracle.exact(manifest.get("schema_version"), 1)
            and manifest.get("upstream_revision") == oracle.REVISION, "schema/revision")
    require(oracle.exact(manifest.get("coverage"), COVERAGE), "capture scope")
    require(manifest.get("generator") == HELPER and manifest.get("generator_sha256") == PINS[HELPER], "helper provenance")
    require(oracle.exact(manifest.get("repository_input_sha256"), {key: PINS[key] for key in (DRIVER, CAPTURE)}), "driver provenance")
    require(manifest.get("importer") == CHECKER and manifest.get("importer_sha256") == sha(ROOT / CHECKER), "checker identity")
    catalog = []
    for name in sorted(fixture_names):
        path = collection / name
        require(path.stat().st_size <= oracle.MAX_RECORDS, "fixture size cap")
        catalog.append({"path": name, "size": path.stat().st_size, "sha256": sha(path)})
    require(oracle.exact(manifest.get("fixtures"), catalog), "fixture catalog")
    digest = hashlib.sha256(json.dumps(catalog, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    require(digest == CATALOG_SHA256 == manifest.get("fixture_catalog_sha256"), "independently pinned fixture catalog")
    inventory = oracle.decode_json((ROOT / "tests/upstream/manifest.json").read_bytes())
    require(inventory["upstream"]["revision"] == oracle.REVISION, "inventory revision")
    expected = {row["path"]: row["sha256"] for row in inventory["sources"] + inventory["excluded_sources"]
                if row["path"].startswith("lib/") and Path(row["path"]).suffix in (".c", ".h")}
    require(oracle.exact(manifest.get("source_sha256"), expected), "complete recorded lib C/header closure")
    require(oracle.exact(read_json("recipes.json"), {"schema_version": 1, "status": "pre-execution source-only hypotheses",
                         "recipes": [case.metadata() for case in cases], "case_count": len(cases)}), "independent recipes")
    results = [oracle.validate(case, collection / ("normal-" + case.name + ".stdout")) for case in cases]
    capture = manifest.get("capture")
    require(type(capture) is dict and all(oracle.exact(rows[0], capture.get("metadata")) for rows in results), "host metadata")
    require(capture.get("normal_sanitizer_equal") is True
            and oracle.exact(capture.get("c_case_executions"), 60)
            and oracle.exact(capture.get("instrumentation"), {
                "helper_and_unchanged_double_fast_and_inline_headers": True,
                "remaining_archive_instrumented": False, "leak_sanitizer": False}), "instrumentation scope")
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", required=True)
    parser.add_argument("--collection", type=Path, default=COLLECTION)
    args = parser.parse_args()
    check(args.collection)
    print("Validated 15 direct DoubleFast recipes / 30 observations offline; no subprocess was used.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError) as error:
        print("ERROR: " + str(error), file=sys.stderr)
        raise SystemExit(1)
