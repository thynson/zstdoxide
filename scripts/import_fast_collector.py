#!/usr/bin/env python3
"""Validate frozen Fast collector observations offline or reproduce them with C.

The three original helper/driver/control files stay byte-identical. This adapter
adds repository identity checks and exact frozen-record comparison. --check never
starts a compiler, the reference executable, or any other subprocess.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
COLLECTION = ROOT / "tests/upstream/fast-collector"
GENERATOR = "tests/upstream/fast-collector/collector_oracle.c"
DRIVER = "tests/upstream/fast-collector/collector.py"
CONTROLS = "tests/upstream/fast-collector/negative_controls.py"
IMPORTER = "scripts/import_fast_collector.py"
GENERATOR_SHA256 = "f5b575b5d2b57191a210e71402ade98b7e5039fcbc13115165fd95b2548308af"
DRIVER_SHA256 = "f393b832243b9d104d9249a79ddecf4c8a8ed2e710f8a5d6460a7135c37d0f99"
CONTROLS_SHA256 = "ed4976c953c7787ca6d7d2e6b0dbf1cd74fe81ae20d88587e00b4df39ca95a35"
CATALOG_SHA256 = "5cfe36886aeb6692f8c19a45a944c1058c015923541e990d085ecd7fda8f58d6"
REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
COVERAGE = {
    "kind": "bounded-fast-collector-corroboration", "base_recipes": 40,
    "capacity_variants": 6, "cases": 46, "observations": 92,
    "padding_patterns": [106, 195], "entry_point": "ZSTD_generateSequences",
    "direct_comparison": "ZSTD_compressBlock_fast", "ordinary_frames": False,
}
INSTRUMENTATION = {
    "helper_compress_c_fast_c_inline_headers": True,
    "remaining_archive_instrumented": False, "leak_sanitizer": False,
}


def require(ok, reason):
    if not ok:
        raise ValueError(reason)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_driver():
    path = ROOT / DRIVER
    # Verify the imported code before executing module definitions. A detached
    # test-support fixture must not run arbitrary modified Python during --check.
    require(path.is_file() and sha(path) == DRIVER_SHA256, "frozen replay driver before import")
    spec = importlib.util.spec_from_file_location("zstdoxide_fast_collector_driver", path)
    require(spec is not None and spec.loader is not None, "driver module specification")
    driver = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = driver
    spec.loader.exec_module(driver)
    return driver


def load_json(driver, path, limit=2 << 20):
    require(path.is_file() and path.stat().st_size <= limit, "bounded JSON metadata")
    return driver.decode_json(path.read_text())


def offline_check(collection, driver):
    collection = collection.resolve()
    require(collection.is_dir(), "missing collector collection")
    base_cases = driver.recipes()
    derived_names = [parent + "-capacity-" + label
                     for parent in ("width-4", "history-period")
                     for label in ("exact", "short", "zero")]
    case_names = [case.name for case in base_cases] + derived_names
    expected = {"normal-" + name + ".stdout" for name in case_names} | {
        "recipes.json", "capacity-recipes.json", "collector.py", "negative_controls.py",
    }
    actual = {p.relative_to(collection).as_posix() for p in collection.rglob("*") if p.is_file()}
    require(actual == expected | {"manifest.json", "README.md", "NOTICE", "collector_oracle.c"},
            "missing or unlisted collector collection files")
    for name in actual:
        require((collection / name).resolve().is_relative_to(collection), "collection path escapes")
    manifest = load_json(driver, collection / "manifest.json")
    require(type(manifest) is dict and type(manifest.get("schema_version")) is int
            and manifest["schema_version"] == 1 and manifest.get("upstream_revision") == REVISION,
            "collector manifest schema/revision")
    exact = driver.exact
    require(exact(manifest.get("coverage"), COVERAGE), "collector coverage scope")
    require(manifest.get("generator") == GENERATOR
            and manifest.get("generator_sha256") == GENERATOR_SHA256
            and sha(ROOT / GENERATOR) == GENERATOR_SHA256
            and sha(collection / "collector_oracle.c") == GENERATOR_SHA256, "frozen C helper")
    require(exact(manifest.get("repository_input_sha256"),
                  {DRIVER: DRIVER_SHA256, CONTROLS: CONTROLS_SHA256}), "frozen support identities")
    require(sha(ROOT / CONTROLS) == CONTROLS_SHA256, "frozen original negative controls")
    require(manifest.get("importer") == IMPORTER
            and manifest.get("importer_sha256") == sha(Path(__file__)), "current importer hash")
    catalog = []
    for name in sorted(expected):
        path = collection / name
        require(path.stat().st_size <= driver.MAX_RECORD_BYTES, "fixture file size cap")
        catalog.append({"path": name, "size": path.stat().st_size, "sha256": sha(path)})
    require(exact(manifest.get("fixtures"), catalog), "exact collector fixture catalog")
    digest = hashlib.sha256(json.dumps(catalog, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    require(digest == CATALOG_SHA256 == manifest.get("fixture_catalog_sha256"),
            "frozen collector observation and source identities")
    source_inventory = load_json(driver, ROOT / "tests/upstream/manifest.json", 16 << 20)
    require(source_inventory.get("upstream", {}).get("revision") == REVISION, "source inventory revision")
    source_hashes = {row["path"]: row["sha256"] for row in
                     source_inventory["sources"] + source_inventory["excluded_sources"]}
    source = manifest.get("source_sha256")
    require(type(source) is dict and {
        "lib/zstd.h", "lib/zstd_errors.h", "lib/compress/zstd_compress.c",
        "lib/compress/zstd_compress_internal.h", "lib/compress/zstd_fast.c",
        "lib/compress/zstd_fast.h", "lib/common/zstd_internal.h", "lib/common/mem.h",
    } <= set(source), "required pinned collector sources")
    for name, digest in source.items():
        require(type(name) is str and source_hashes.get(name) == digest, "pinned source hash: " + str(name))
    require(exact(load_json(driver, collection / "recipes.json"), {
        "schema_version": 1, "phase": "before any C execution", "generator_sha256": DRIVER_SHA256,
        "case_count": 40, "cases": [case.metadata() for case in base_cases],
        "capacity_plan": ["width-4", "history-period"],
        "capacity_variants": ["observed exact count", "observed count minus one", "zero"],
        "qualification": "Capacity fields only depend on first observed normal successful row counts. Source bytes remain independently frozen.",
    }), "independently reconstructed base recipes")
    validated = [driver.validate(case, collection / ("normal-" + case.name + ".stdout"))
                 for case in base_cases]
    derived = []
    for parent in ("width-4", "history-period"):
        original = next(case for case in base_cases if case.name == parent)
        observation = next(row for row in validated if row["id"] == parent)
        require(observation["error"] == 0 and type(observation["rows"]) is int
                and observation["rows"] > 0, "capacity parent observed success")
        count = observation["rows"]
        for label, capacity in (("exact", count), ("short", count - 1), ("zero", 0)):
            derived.append(driver.replace(original, name=parent + "-capacity-" + label,
                                          family="capacity", capacity=capacity, derived_from=parent))
    capacity_manifest = load_json(driver, collection / "capacity-recipes.json")
    snapshot = "/private/tmp/zstdoxide-fast-collector-final"
    require(exact(capacity_manifest, {
        "phase": "after normal baseline, before capacity or sanitizer calls",
        "cases": [case.metadata() for case in derived],
        "packet_pins": [{"path": snapshot + "/packets/" + case.name + ".packet",
                         "sha256": hashlib.sha256(case.packet()).hexdigest()} for case in derived],
    }), "observed-count capacity derivation and historical packet pins")
    validated.extend(driver.validate(case, collection / ("normal-" + case.name + ".stdout"))
                     for case in derived)
    cases = base_cases + derived
    capture = manifest.get("capture")
    require(type(capture) is dict and capture.get("source_snapshot") == snapshot,
            "original capture location is provenance only")
    for case in cases:
        path = collection / ("normal-" + case.name + ".stdout")
        metadata = driver.decode_json(path.read_text().splitlines()[0])
        require(exact(metadata, capture.get("metadata")), "captured metadata bound to every case")
    require(exact(capture.get("cases"), validated), "observed outcome/corroboration summary")
    require(capture.get("evidence") == "docs/evidence/2026-10-09-fast-collector.json"
            and capture.get("original_evidence_sha256") == "232daa52a8ddfc63a03209d3155478aa323f8437c4af5e8072c482ad8e93a97f"
            and capture.get("original_archive_sha256") == "33923ded89357b0b0e4868c8dcd9de8c650101f810b0f89fbf34d5bda5865121",
            "original immutable evidence references")
    require(capture.get("normal_sanitizer_equal") is True
            and type(capture.get("final_c_case_executions")) is int
            and capture["final_c_case_executions"] == 184
            and capture.get("original_driver_sha256") == DRIVER_SHA256
            and exact(capture.get("instrumentation"), INSTRUMENTATION), "capture counts/instrumentation")
    return manifest, cases


def regenerate(args, collection, manifest, cases, driver):
    require(collection == COLLECTION.resolve(), "regeneration uses the repository collection; --collection overrides are offline only")
    output = args.output.resolve()
    upstream = args.upstream.resolve()
    protected = (ROOT, collection, upstream)
    require(not output.exists(), "preserve existing output; choose a fresh directory")
    for path in protected:
        require(not output.is_relative_to(path) and not path.is_relative_to(output), "regeneration output overlaps protected source")
    report = args.report.resolve() if args.report is not None else None
    if report is not None:
        require(not report.exists(), "preserve existing report")
        for path in (*protected, output):
            require(not report.is_relative_to(path) and not path.is_relative_to(report), "report overlaps protected source or output")
    source_before = {path: sha(ROOT / path) for path in (GENERATOR, DRIVER, CONTROLS, IMPORTER)}
    fixture_before = {row["path"]: row["sha256"] for row in manifest["fixtures"]}
    args.source_manifest = collection / "manifest.json"
    driver.run(args)
    evidence_path = output / "evidence.json"
    evidence = load_json(driver, evidence_path, 16 << 20)
    try:
        comparisons = []
        for mode in (("normal", "sanitized") if args.sanitize else ("normal",)):
            for case in cases:
                saved = [driver.decode_json(line) for line in
                         (collection / ("normal-" + case.name + ".stdout")).read_text().splitlines()]
                observed = [driver.decode_json(line) for line in
                            (output / (mode + "-" + case.name + ".stdout")).read_text().splitlines()]
                require(driver.exact(saved[1:], observed[1:]), "exact frozen public/direct records: " + case.name)
                host = {"size_t_bytes", "little_endian", "sequence_bytes"}
                require(driver.exact({k: v for k, v in saved[0].items() if k not in host},
                                     {k: v for k, v in observed[0].items() if k not in host}), "stable collector metadata")
            comparisons.append({"mode": mode, "cases": 46, "observations": 92,
                                "complete_public_and_corroboration_records_equal": True})
        require(source_before == {path: sha(ROOT / path) for path in source_before}, "repository oracle source changed")
        require(all(sha(collection / path) == digest for path, digest in fixture_before.items()), "frozen fixtures changed")
        evidence["frozen_fixture_comparison"] = comparisons
        evidence["frozen_fixture_catalog_sha256"] = CATALOG_SHA256
        evidence["repository_source_sha256"] = source_before
        evidence["host_qualification"] = "Complete logical records compared exactly; host metadata independently validated. This execution is native only on its recorded compiler target."
        driver.write_json(evidence_path, evidence)
        if report is not None:
            report.parent.mkdir(parents=True, exist_ok=True)
            driver.write_json(report, evidence)
        print("All frozen collector records reproduced; evidence: " + str(evidence_path))
    except BaseException as error:
        evidence["status"] = "failed"
        evidence["frozen_fixture_comparison_error"] = str(error)
        driver.write_json(evidence_path, evidence)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--collection", type=Path, default=COLLECTION)
    parser.add_argument("--upstream", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--cc")
    parser.add_argument("--sanitize", action="store_true")
    args = parser.parse_args()
    if args.check:
        require(args.upstream is None and args.output is None and args.report is None and args.cc is None and not args.sanitize,
                "--check is offline and cannot include regeneration options")
    else:
        require(args.upstream is not None and args.output is not None,
                "use --check or supply --upstream and --output")
        if args.cc is None:
            args.cc = "cc"
    driver = load_driver()
    collection = args.collection.resolve()
    manifest, cases = offline_check(collection, driver)
    if args.check:
        print("Validated 46 Fast collector cases / 92 padding observations offline; no subprocess was used.")
    else:
        regenerate(args, collection, manifest, cases, driver)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, OverflowError) as error:
        print("ERROR: " + str(error), file=sys.stderr)
        raise SystemExit(1)
