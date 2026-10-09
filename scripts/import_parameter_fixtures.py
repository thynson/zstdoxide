#!/usr/bin/env python3
"""Freeze v1.5.7 scalar parameter/context observations; never a codec binding.

--check regenerates exact TSV bytes and stable provenance, while reporting the
current compiler/archive independently of original capture metadata. Only a
64-bit size_t / 32-bit int C host is accepted; 32-bit bounds are macro-derived.
"""

import argparse
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import shlex
import subprocess
import tempfile

REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "tests/upstream/parameters"
ORACLE = DESTINATION / "oracle.c"
SECTIONS = ("presets", "bounds", "width-bounds", "queries", "adjustments", "requests", "lifecycle")
SOURCES = ("lib/zstd.h", "lib/zstd_errors.h", "lib/common/zstd_internal.h",
           "lib/compress/zstd_compress_internal.h", "lib/compress/clevels.h",
           "lib/compress/zstd_compress.c", "tests/fuzzer.c", "tests/zstreamtest.c")
DICTIONARY = ROOT / "tests/upstream/dictionaries/formatted.dict"
FIELDS = ("window_log", "chain_log", "hash_log", "search_log", "min_match", "target_length", "strategy")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def rows(data):
    return list(csv.DictReader(io.StringIO(data.decode("ascii")), delimiter="\t"))


def validate(outputs):
    tables = {section: rows(outputs[section + ".tsv"]) for section in SECTIONS}
    for section, records in tables.items():
        if not records or any(None in row or None in row.values() for row in records):
            raise ValueError(f"Empty/malformed column data: {section}")
    assert len(tables["presets"]) == 92
    assert {(int(r["tier"]), int(r["row"])) for r in tables["presets"]} == {
        (tier, level) for tier in range(4) for level in range(23)}
    assert len(tables["bounds"]) == 13
    assert len(tables["width-bounds"]) == 2
    assert all(r["validation_error"] == "0" for r in tables["queries"] + tables["adjustments"])
    expected_levels = {-(1 << 31), -131073, -131072, -999, -9, -1, 0, *range(1, 24), (1 << 31) - 1}
    assert {int(r["level"]) for r in tables["queries"] if r["case"] == "level_tiers"} == expected_levels
    for key in FIELDS:
        values = {int(r["input_" + key]) for r in tables["adjustments"] if r["case"] == key}
        assert 0xffffffff in values and 0x80000000 in values
    empty = next(r for r in tables["queries"] if r["case"] == "typed_empty_or_unknown" and r["level"] == "3" and r["source_kind"] == "known")
    unknown = next(r for r in tables["queries"] if r["case"] == "size_resolution" and r["level"] == "3" and r["source"] == "0")
    assert tuple(empty[key] for key in FIELDS) != tuple(unknown[key] for key in FIELDS)
    hint = [r for r in tables["requests"] if r["scenario"] == "source_hint_small"]
    assert len(hint) == 2 and hint[1]["source_hint"] == "16384" and hint[1]["error_code"] == "0"
    for prefix in ("empty_", "known_"):
        assert {key: value for key, value in hint[0].items() if key.startswith(prefix)} == {
            key: value for key, value in hint[1].items() if key.startswith(prefix)}
    assert {key: value for key, value in hint[0].items() if key.startswith("unknown_")} != {
        key: value for key, value in hint[1].items() if key.startswith("unknown_")}
    assert any(r["operation"] == "bulk_fresh" and r["scenario"] == "bulk_cdict" and r["error_code"] != "0" for r in tables["lifecycle"])
    assert any(r["operation"] == "bulk_active" and r["error_code"] != "0" for r in tables["lifecycle"])
    partial = [r for r in tables["lifecycle"] if r["operation"] == "partial_header"]
    assert len(partial) == 3 and all(r["produced"] == "1" and r["header_present"] == "0" for r in partial)
    errors = [r for r in tables["lifecycle"] if r["operation"] == "pledge_error"]
    assert len(errors) == 3 and all(r["error_code"] == "72" for r in errors)
    first_end = next(r for r in tables["lifecycle"] if r["operation"] == "first_end")
    assert first_end["error_code"] == "0" and first_end["header_content_size"] == "2"
    return {section: len(records) for section, records in tables.items()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--report", type=Path, help="save current regeneration metadata outside the fixture capture")
    args = parser.parse_args()
    upstream = args.upstream.resolve()
    revision = subprocess.check_output(["git", "-C", str(upstream), "rev-parse", "HEAD"], text=True).strip()
    if revision != REVISION:
        parser.error(f"expected {REVISION}, got {revision}")
    subprocess.run(["git", "-C", str(upstream), "diff", "--quiet", "HEAD", "--", "lib", "tests"], check=True)
    library = upstream / "lib/libzstd.a"
    if not library.is_file():
        parser.error("build the pinned external lib/libzstd.a first")
    compiler = shlex.split(os.environ.get("CC", "cc"))
    flags = ["-std=c99", "-O2", "-Wall", "-Wextra", "-Werror"]
    outputs = {}
    with tempfile.TemporaryDirectory(prefix="zstdoxide-parameters-") as temporary:
        executable = Path(temporary) / "oracle"
        command = [*compiler, *flags, "-I", str(upstream / "lib"), str(ORACLE), str(library), "-pthread", "-o", str(executable)]
        subprocess.run(command, check=True)
        for section in SECTIONS:
            arguments = [str(executable), section]
            if section == "lifecycle":
                arguments.append(str(DICTIONARY))
            outputs[section + ".tsv"] = subprocess.check_output(arguments)
    counts = validate(outputs)
    capture = {
        "compiler_command": compiler,
        "compiler_version": subprocess.check_output([*compiler, "--version"], text=True),
        "compile_command_template": [*compiler, *flags, "-I", "<upstream>/lib", "<root>/tests/upstream/parameters/oracle.c", "<upstream>/lib/libzstd.a", "-pthread", "-o", "<temporary>/oracle"],
        "host_system": platform.system(), "host_machine": platform.machine(),
        "executed_size_t_bits": 64, "executed_int_bits": 32,
        "static_archive_sha256": digest(library.read_bytes()),
        "policy": "Compiler/archive/host identify this capture; --check compares stable data/provenance and reports current execution metadata separately.",
    }
    stable = {
        "schema_version": 1, "upstream_revision": REVISION,
        "generator": str(ORACLE.relative_to(ROOT)), "generator_sha256": digest(ORACLE.read_bytes()),
        "importer": str(Path(__file__).resolve().relative_to(ROOT)), "importer_sha256": digest(Path(__file__).read_bytes()),
        "source_sha256": {name: digest((upstream / name).read_bytes()) for name in SOURCES},
        "repository_input_sha256": {str(DICTIONARY.relative_to(ROOT)): digest(DICTIONARY.read_bytes())},
        "fixtures": [{"path": name, "size": len(data), "sha256": digest(data), "rows": counts[name[:-4]]} for name, data in sorted(outputs.items())],
        "policy": "Test-only C parameter oracle. All seven compression fields are recorded. Public zero-source conversion, typed-no-attach adapter, reserved/modular C cases, and lifecycle observations are distinct. No operational numeric Rust levels or C lifecycle parity is claimed.",
    }
    manifest_path = DESTINATION / "manifest.json"
    if args.check:
        recorded = json.loads(manifest_path.read_text())
        if {key: value for key, value in recorded.items() if key != "capture"} != stable:
            raise ValueError("parameter stable provenance differs; intentional fixture updates require regeneration")
        for name, data in outputs.items():
            if (DESTINATION / name).read_bytes() != data:
                raise ValueError(f"parameter fixture bytes differ: {name}")
    else:
        DESTINATION.mkdir(parents=True, exist_ok=True)
        for name, data in outputs.items():
            (DESTINATION / name).write_bytes(data)
        manifest_path.write_text(json.dumps({**stable, "capture": capture}, indent=2) + "\n")
    report = {"mode": "check" if args.check else "generate", "revision": REVISION,
              "rows": counts, "capture": capture, "stable_provenance_sha256": digest(json.dumps(stable, sort_keys=True).encode()),
              "fixtures": stable["fixtures"]}
    if args.report:
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"{'Verified' if args.check else 'Generated'} parameter fixtures: " + json.dumps(counts, sort_keys=True))
    print("Executed 64-bit C with 32-bit int; 32-bit target bounds are macro-derived, not a native execution.")


if __name__ == "__main__":
    main()
