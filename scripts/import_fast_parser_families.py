#!/usr/bin/env python3
"""Check frozen direct Fast families offline or regenerate them with pinned C."""

from pathlib import Path
import argparse
import hashlib
import json
import sys

from oracles import fast_direct_families as families

ROOT = Path(__file__).resolve().parents[1]
COLLECTION = ROOT / "tests/upstream/fast-parser-families"
GENERATOR = "scripts/oracles/fast_direct_families.c"
DRIVER = "scripts/oracles/fast_direct_families.py"
IMPORTER = "scripts/import_fast_parser_families.py"
GENERATOR_SHA256 = "7001f317a95274e8b504b80abb755da386c8f5036f9600a298d08840a2a2c20b"
DRIVER_SHA256 = "f7c6f09ad37ffe63f8952bbf3fc5bc5d30464488d87b19a8ff6be64cc10c6419"
CATALOG_SHA256 = "acb3b9de9e647e15bd81cc563b144f010046239b7cc8f8b7a29b783942ebfd2d"
COVERAGE = {
    "kind": "bounded-direct-families", "recipes": 142, "observations": 284,
    "padding_patterns": [106, 195], "entry_point": "ZSTD_compressBlock_fast",
    "collector": False, "ordinary_frames": False,
}
require = families.require
exact = families.exact
sha = families.sha


def load_json(path, limit=2 << 20):
    require(path.is_file() and path.stat().st_size <= limit, "bounded JSON metadata")
    return families.decode_json(path.read_text())


def offline_check(collection):
    collection = collection.resolve()
    require(collection.is_dir(), "missing family collection")
    cases = families.recipes()
    expected = {"normal-" + case.name + ".stdout" for case in cases} | {"recipes.json"}
    actual = {p.relative_to(collection).as_posix() for p in collection.rglob("*") if p.is_file()}
    require(actual == expected | {"manifest.json", "README.md", "NOTICE"},
            "missing or unlisted collection files")
    for name in actual:
        require((collection / name).resolve().is_relative_to(collection), "fixture escapes collection")
    manifest = load_json(collection / "manifest.json")
    require(type(manifest) is dict and type(manifest.get("schema_version")) is int
            and manifest["schema_version"] == 1
            and manifest.get("upstream_revision") == families.REVISION, "schema/revision")
    require(exact(manifest.get("coverage"), COVERAGE), "family coverage scope")
    require(manifest.get("generator") == GENERATOR
            and manifest.get("generator_sha256") == GENERATOR_SHA256
            and sha(ROOT / GENERATOR) == GENERATOR_SHA256, "frozen C helper")
    require(exact(manifest.get("repository_input_sha256"), {DRIVER: DRIVER_SHA256})
            and sha(ROOT / DRIVER) == DRIVER_SHA256, "frozen recipe and replay driver")
    require(manifest.get("importer") == IMPORTER
            and manifest.get("importer_sha256") == sha(Path(__file__)), "current importer hash")
    catalog = []
    for name in sorted(expected):
        path = collection / name
        require(path.stat().st_size <= families.MAX_RECORDS, "fixture size cap")
        catalog.append({"path": name, "size": path.stat().st_size, "sha256": sha(path)})
    require(exact(manifest.get("fixtures"), catalog), "exact fixture catalog")
    digest = hashlib.sha256(json.dumps(catalog, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    require(digest == CATALOG_SHA256 == manifest.get("fixture_catalog_sha256"),
            "frozen C observation and recipe identities")
    provenance = load_json(ROOT / "tests/upstream/manifest.json", 16 << 20)
    require(provenance.get("upstream", {}).get("revision") == families.REVISION, "source catalog revision")
    source_hashes = {row["path"]: row["sha256"] for row in provenance["sources"] + provenance["excluded_sources"]}
    sources = manifest.get("source_sha256")
    require(type(sources) is dict and {
        "lib/zstd.h", "lib/compress/zstd_fast.c", "lib/compress/zstd_fast.h",
        "lib/compress/zstd_compress.c", "lib/compress/zstd_compress_internal.h",
        "lib/common/zstd_internal.h", "lib/common/mem.h",
    } <= set(sources), "required pinned sources")
    for name, digest in sources.items():
        require(type(name) is str and source_hashes.get(name) == digest, "pinned source hash: " + str(name))
    recipe_manifest = load_json(collection / "recipes.json")
    require(exact(recipe_manifest, {
        "schema_version": 2, "source_generator_sha256": DRIVER_SHA256,
        "cases": [case.metadata() for case in cases], "case_count": 142,
        "observation_count": 284, "status": "pre-execution deterministic recipe hypotheses",
    }), "independently reconstructed deterministic recipes")
    rows = [families.validate(case, collection / ("normal-" + case.name + ".stdout")) for case in cases]
    capture = manifest.get("capture")
    require(type(capture) is dict and all(exact(row["metadata"], capture.get("metadata")) for row in rows),
            "captured host metadata agrees with every observation")
    review = families.compare_families(cases, collection, "normal", rows)
    require(exact(capture.get("family_review"), review), "recorded family witnesses")
    require(capture.get("normal_sanitizer_equal") is True
            and type(capture.get("final_c_case_executions")) is int
            and capture["final_c_case_executions"] == 568
            and capture.get("original_driver_sha256") == DRIVER_SHA256
            and exact(capture.get("instrumentation"), {
                "helper_and_unchanged_fast_and_inline_headers": True,
                "remaining_archive_instrumented": False, "leak_sanitizer": False,
            }), "capture count and instrumentation qualifications")
    return manifest, cases


def regenerate(args, collection, manifest, cases):
    output = args.output.resolve()
    # Resolve and check before creating anything: the backend must never place
    # compiler output inside source, repository, or the frozen fixture collection.
    upstream = args.upstream.resolve()
    for protected in (ROOT, collection, upstream):
        require(not output.is_relative_to(protected) and not protected.is_relative_to(output),
                "regeneration output must be disjoint from repository, collection and reference")
    require(not output.exists(), "preserve existing output; choose a fresh directory")
    source_before = {name: sha(ROOT / name) for name in (GENERATOR, DRIVER, IMPORTER)}
    fixture_before = {row["path"]: row["sha256"] for row in manifest["fixtures"]}
    args.source_manifest = collection / "manifest.json"
    families.run(args)
    report_path = output / "evidence.json"
    evidence = load_json(report_path, 16 << 20)
    try:
        comparisons = []
        for mode in (("normal", "sanitized") if args.sanitize else ("normal",)):
            for case in cases:
                saved = [families.decode_json(line) for line in
                         (collection / ("normal-" + case.name + ".stdout")).read_text().splitlines()]
                observed_path = output / (mode + "-" + case.name + ".stdout")
                observed = [families.decode_json(line) for line in observed_path.read_text().splitlines()]
                require(exact(saved[1:], observed[1:]), "complete frozen case records: " + case.name)
                host = {"pointer_bits", "little_endian", "sizeof_match_state"}
                require(exact({k: v for k, v in saved[0].items() if k not in host},
                              {k: v for k, v in observed[0].items() if k not in host}), "stable metadata")
            comparisons.append({"mode": mode, "recipes": len(cases), "observations": 2 * len(cases),
                                "all_case_records_equal": True})
        require(source_before == {name: sha(ROOT / name) for name in source_before}, "repository source changed")
        require(all(sha(collection / name) == digest for name, digest in fixture_before.items()), "fixtures changed")
        evidence["frozen_fixture_comparison"] = comparisons
        evidence["repository_source_sha256"] = source_before
        evidence["frozen_fixture_catalog_sha256"] = CATALOG_SHA256
        evidence["host_qualification"] = "Logical records compared exactly; native layout observed independently."
        families.write_json(report_path, evidence)
        print("All frozen family observations reproduced; evidence: " + str(report_path))
    except BaseException as error:
        evidence["status"] = "failed"
        evidence["frozen_fixture_comparison_error"] = str(error)
        families.write_json(report_path, evidence)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--collection", type=Path, default=COLLECTION)
    parser.add_argument("--upstream", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--cc", default="cc")
    parser.add_argument("--sanitize", action="store_true")
    args = parser.parse_args()
    if args.check:
        require(args.upstream is None and args.output is None and not args.sanitize,
                "offline check cannot include regeneration options")
    else:
        require(args.upstream is not None and args.output is not None,
                "use --check or supply --upstream and --output")
    collection = args.collection.resolve()
    manifest, cases = offline_check(collection)
    if args.check:
        print("Validated 142 direct Fast recipes / 284 observations offline; no C subprocess was used.")
    else:
        regenerate(args, collection, manifest, cases)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError) as error:
        print("ERROR: " + str(error), file=sys.stderr)
        raise SystemExit(1)
