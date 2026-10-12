#!/usr/bin/env python3
"""Verify complete frozen DoubleFast family observations without executing C."""

from dataclasses import replace
from pathlib import Path
import argparse
import hashlib
import json
import sys

ROOT = Path(__file__).resolve().parents[1]
COLLECTION = ROOT / "tests/upstream/double-fast-families"
HELPER = "scripts/oracles/double_fast_direct.c"
DRIVER = "scripts/oracles/double_fast_direct.py"
GENERATOR = "scripts/oracles/double_fast_families.py"
CAPTURE = "scripts/capture_double_fast_families.py"
CHECKER = "scripts/check_double_fast_families.py"
BASE = "tests/fixtures/reference-double-fast-recipes/recipes.json"
REFINEMENT = "tests/fixtures/reference-double-fast-recipes/refinement-long-literal.json"
PINS = {
    HELPER: "e20d452423a3f913831ac04f6edd79be3d0d18fb0fe6609357ef4ea47816d9d4",
    DRIVER: "5280b94e6e10e34a263b7ae1b31a24026adfd92327767b776e81344362604065",
    GENERATOR: "56fa1aa0df89cb173c596c94d40ac8aa08c183b2dc8af50c9cc128308877e15e",
    CAPTURE: "baf740484735cfd1de291fd1450390172e6e964d5ab64261e537be3d4caad34b",
    "scripts/check_double_fast_recipes.py": "5ea9dbaa4aac38d6eb64b7379a9684c61247bbb9a560a2ae3728d392bdca710a",
    "scripts/capture_double_fast_tiny.py": "5dea723222db1171e6302411d4d59870e01a09dfd7df74bf47c51cc6978bef19",
    BASE: "b2e9b2c8302e99a5974ea5d9d8fba67986c4f3cd94a34810631fa56c04d811e7",
    REFINEMENT: "db2009c5cbadde7d7f244ff2f46eed879005ed0fba3379258bff4122f8c2acf2",
}
CATALOG_SHA256 = "e8e48567163737ce8856f07dbca8eea7a30b499e7aefe29b0f9107481756ec0a"
COVERAGE = {"base_recipes": 168, "refinements": 1, "recipes": 169, "observations": 338,
            "padding_patterns": [106, 195], "collector": False, "ordinary_frames": False,
            "chronological_trace": False, "entry_point": "ZSTD_compressBlock_doubleFast"}


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_cases():
    # Inputs and imported Python identities are pinned independently of manifests.
    for name, digest in PINS.items():
        require(sha(ROOT / name) == digest, "frozen input identity: " + name)
    import check_double_fast_recipes as checker
    cases = [case.recipe for case in checker.check_manifest(ROOT / BASE)]
    oracle = checker.load_families().direct
    original = next(case for case in cases if case.name == "long-literal-candidate")
    refined = replace(original, name="long-literal-extended-tail", source=original.source + b"A" * 896)
    refinement = oracle.decode_json((ROOT / REFINEMENT).read_bytes())
    require(oracle.exact(refinement["recipe"], refined.metadata())
            and oracle.exact(refinement["change"], {"appended_byte": 65, "appended_count": 896}), "independent refinement input")
    cases.append(refined)
    require(len(cases) == len({case.name for case in cases}) == 169, "combined case cardinality")
    return oracle, cases


def review(oracle, cases, records):
    """Checked consequences of isolated sources; never a chronological trace."""
    pairs = []

    def equivalent(left, right, ignored):
        a, b = records[left], records[right]
        require(oracle.exact({k: v for k, v in a.items() if k not in ignored},
                             {k: v for k, v in b.items() if k not in ignored}), "family equivalence: " + left + "/" + right)
        pairs.append({"left": left, "right": right, "ignored_input_fields": sorted(ignored)})

    for source in ("period", "split", "asymmetric"):
        for window in (18, 21):
            equivalent(f"width-{source}-m3-w{window}", f"width-{source}-m4-w{window}", {"id", "min_match"})
        for width in range(3, 8):
            equivalent(f"width-{source}-m{width}-w18", f"width-{source}-m{width}-w21", {"id", "window_log"})
    equivalent("seed-order-repeat-first", "seed-order-last-first", {"id", "long_seeds", "short_seeds"})

    def sequences(name, expected):
        actual = [[s["ll"], s["ml"], s["distance"], s["off_base"]] for s in records[name]["sequences"]]
        require(oracle.exact(actual, expected), "observed sequence witness: " + name)
        return actual

    witnesses = {}
    expected_sequences = {
        "conditional-store-step3": [[512, 4, 544, 547]],
        "conditional-store-step4": [[770, 4, 802, 805]],
        "lookahead-length-11": [[0, 12, 64, 67]],
        "lookahead-length-12": [[0, 12, 64, 67]],
        "lookahead-length-13": [[1, 13, 33, 36]],
        "current-long-priority-seeded": [[0, 16, 64, 67]],
        "current-long-priority-empty": [[0, 20, 32, 35]],
        "adjacent-long-colliding": [[0, 4, 32, 35]],
        "adjacent-long-control": [[1, 8, 48, 51]],
        "repeats-loop-available": [[0, 32, 96, 99], [0, 32, 32, 1], [0, 32, 96, 1], [0, 32, 32, 1]],
        "repeat-bound-1024": [[3, 29, 1024, 1]],
        "repeat-bound-1025": [],
        "long-literal-candidate": [],
        "long-literal-extended-tail": [[65554, 64, 65618, 65621], [0, 942, 2, 5]],
    }
    for role in ("long", "short"):
        expected_sequences["floor-" + role + "-539"] = []
        expected_sequences["floor-" + role + "-540"] = [[0, 16, 960, 963]]
        expected_sequences["floor-" + role + "-541"] = [[0, 16, 959, 962]]
    for name, expected in expected_sequences.items():
        witnesses[name] = sequences(name, expected)
    deferred = []
    by_name = {case.name: case for case in cases}
    for step, position, expected_bucket, expected_cell in ((3, 512, 215372, 549), (4, 770, 69048, 0)):
        name = "conditional-store-step" + str(step)
        case, row = by_name[name], records[name]
        target = case.prefix + position + step
        slot = oracle.bucket(case.source[target:target + 8], case.long_log)
        table = row["long_table"]
        before = dict(table["before"])
        after = before | {bucket: new for bucket, old, new in table["changes"]}
        require(slot == expected_bucket and before.get(slot, 0) == 0 and after.get(slot, 0) == expected_cell, "deferred final bucket witness")
        deferred.append({"id": name, "bucket": slot, "candidate_raw_index": target + 2, "final_raw_cell": expected_cell})
    markers = []
    for name, kind, raw_ll, raw_ml in (("long-match-65540", 0, 2, 65535),
                                      ("long-match-65541", 2, 2, 0),
                                      ("long-literal-extended-tail", 1, 18, 61)):
        row = records[name]
        first = row["sequences"][0]
        require((row["long_length_type"], row["long_length_position"], first["raw_ll"], first["raw_ml_base"]) == (kind, 0, raw_ll, raw_ml), "long-marker witness")
        markers.append({"id": name, "kind": kind, "position": 0, "raw_ll": raw_ll, "raw_ml_base": raw_ml})
    original = records["long-literal-candidate"]
    require(original["long_length_type"] == 0 and original["trailing_literals"] == 65664, "preserved original no-match result")
    last_sample = max(row[2] for row in original["long_table"]["changes"]) - 2 - original["prefix_bytes"]
    require(last_sample == 65298, "original final sampled position")
    return {"equivalence_pairs": pairs, "sequence_witnesses": witnesses, "deferred_final_cells": deferred,
            "long_markers": markers, "unmet_original_hypothesis": {"id": "long-literal-candidate",
            "sequence_count": 0, "last_final_sample_relative": last_sample,
            "matching_tail_starts_at": 65536, "refinement": "long-literal-extended-tail"},
            "qualification": "Isolated input consequences and complete final tables, not intermediate store chronology or whole-context publication."}


def check(collection):
    oracle, cases = load_cases()
    collection = collection.resolve()
    require(collection.is_dir(), "missing family collection")
    expected_names = {"normal-" + case.name + ".stdout" for case in cases}
    actual = set()
    for path in collection.rglob("*"):
        require(path.is_file() and not path.is_symlink(), "flat regular fixture collection")
        actual.add(path.name)
    require(actual == expected_names | {"manifest.json", "README.md", "NOTICE"}, "exact family file catalog")
    manifest_path = collection / "manifest.json"
    require(manifest_path.stat().st_size <= 2 << 20, "manifest cap")
    manifest = oracle.decode_json(manifest_path.read_bytes())
    require(type(manifest) is dict and oracle.exact(manifest.get("schema_version"), 1)
            and manifest.get("upstream_revision") == oracle.REVISION, "manifest schema/revision")
    require(oracle.exact(manifest.get("coverage"), COVERAGE), "family scope")
    require(manifest.get("generator") == HELPER and manifest.get("generator_sha256") == PINS[HELPER], "helper provenance")
    require(manifest.get("importer") == CHECKER and manifest.get("importer_sha256") == sha(ROOT / CHECKER), "checker identity")
    require(oracle.exact(manifest.get("repository_input_sha256"), {name: digest for name, digest in PINS.items() if name != HELPER}), "all frozen local input provenance")
    catalog = []
    for name in sorted(expected_names):
        path = collection / name
        require(path.stat().st_size <= oracle.MAX_RECORDS, "bounded observation file")
        catalog.append({"path": name, "size": path.stat().st_size, "sha256": sha(path)})
    require(oracle.exact(manifest.get("fixtures"), catalog), "catalog records")
    digest = hashlib.sha256(json.dumps(catalog, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    require(digest == CATALOG_SHA256 == manifest.get("fixture_catalog_sha256"), "independently pinned observations")
    inventory = oracle.decode_json((ROOT / "tests/upstream/manifest.json").read_bytes())
    sources = {r["path"]: r["sha256"] for r in inventory["sources"] + inventory["excluded_sources"]
               if r["path"].startswith("lib/") and Path(r["path"]).suffix in (".c", ".h")}
    require(oracle.exact(manifest.get("source_sha256"), sources), "pinned lib source/header closure")
    capture = manifest.get("capture")
    require(type(capture) is dict and capture.get("normal_sanitized_equal") is True
            and oracle.exact(capture.get("c_case_executions"), 676), "capture cardinality")
    require(oracle.exact(capture.get("instrumentation"), {"helper_and_unchanged_double_fast_and_inline_headers": True,
                                                         "remaining_archive_instrumented": False, "leak_sanitizer": False}), "instrumentation scope")
    records = {}
    for case in cases:
        rows = oracle.validate(case, collection / ("normal-" + case.name + ".stdout"))
        require(oracle.exact(rows[0], capture.get("metadata")), "consistent native metadata")
        records[case.name] = rows[1]
    witnesses = review(oracle, cases, records)
    require(oracle.exact(capture.get("family_review"), witnesses), "checked family witnesses")
    return witnesses


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", required=True)
    parser.add_argument("--collection", type=Path, default=COLLECTION)
    args = parser.parse_args()
    check(args.collection)
    print("Validated 169 direct DoubleFast recipes / 338 observations offline, including the separately frozen refinement.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError) as error:
        print("ERROR: " + str(error), file=sys.stderr)
        raise SystemExit(1)
