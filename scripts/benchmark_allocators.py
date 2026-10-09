#!/usr/bin/env python3
"""Validate and optionally time identical allocator workloads across backends.

The default is clock-free Rust smoke validation. Pass --timed explicitly only
when the host is quiet. Global has no allocation instrumentation. The two
tracking modes use the test helper's Global-backed ledger, not a different heap.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
FEATURES = (
    "unsafe-byte-cursor", "unsafe-bitstream", "unsafe-history", "unsafe-ring-history",
    "unsafe-fse", "unsafe-fse-encoder", "unsafe-huffman", "unsafe-huffman-codebook",
    "unsafe-match-table", "unsafe-match-chain", "unsafe-dictionary", "unsafe-training",
)
CASES = (
    "one_shot_encode", "one_shot_decode", "stream_encode_greedy", "stream_encode_chain",
    "stream_encode_lazy", "stream_decode", "prepare_dictionary", "dictionary_stream_encode",
    "dictionary_stream_decode", "raw_training", "formatted_training",
)
MODES = ("global", "tracking_owned", "tracking_borrowed")
FIXTURES = (
    "tests/upstream/dictionaries/formatted.dict",
    "tests/upstream/dictionaries/training-samples.bin",
    "tests/upstream/data/golden-dictionaries/http-dict-missing-symbols",
    "tests/upstream/data/generated/http-dictionary.zst",
    "tests/upstream/data/golden-compression/http",
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def source_manifest():
    paths = set(ROOT.glob("src/**/*.rs")) | set((ROOT / "test-support/allocator/src").glob("**/*.rs"))
    paths.update(ROOT / name for name in (
        "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "benches/allocators.rs",
        "scripts/benchmark_allocators.py", "test-support/allocator/Cargo.toml", *FIXTURES,
    ))
    digest = hashlib.sha256()
    files = []
    for path in sorted(paths):
        name = path.relative_to(ROOT).as_posix()
        data = path.read_bytes()
        digest.update(name.encode() + b"\0" + data + b"\0")
        files.append({"path": name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    return {"sha256": digest.hexdigest(), "files": files}


def command_output(command):
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=False)
    return {"command": command, "exit_code": result.returncode,
            "stdout": result.stdout, "stderr": result.stderr}


def validate_snapshot(snapshot, label):
    require(isinstance(snapshot, dict), f"Missing snapshot: {label}")
    for field, value in snapshot.items():
        if field == "violations":
            require(value is False, f"Allocator violation: {label}")
        else:
            require(type(value) is int and value >= 0, f"Invalid counter: {label}/{field}")
    require(snapshot["failures"] == 0, f"Unexpected allocation failure: {label}")
    for current, peak in (("live_blocks", "peak_live_blocks"), ("live_bytes", "peak_bytes"),
                          ("live_requested_bytes", "peak_requested_bytes")):
        require(snapshot[current] <= snapshot[peak], f"Invalid high-water mark: {label}/{peak}")


def parse_results(path, configuration, timed):
    records = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    metadata = [row for row in records if row.get("record") == "metadata"]
    require(len(metadata) == 1, f"Expected one metadata record: {path}")
    metadata = metadata[0]
    require(metadata["schema_version"] == 1, f"Unsupported schema: {path}")
    expected_features = {"safe"} if configuration == "safe" else set(FEATURES) if configuration == "all" else {configuration}
    require(set(metadata["features"].split(",")) == expected_features, f"Feature mismatch: {path}")
    require(5 <= metadata["samples"] <= 50, f"Sample count: {path}")
    require(1 <= metadata["minimum_sample_ms"] <= 60_000, f"Minimum duration: {path}")
    expected = {(case, mode) for case in CASES for mode in MODES}
    rows = {}
    for row in records:
        if row.get("record") == "metadata":
            continue
        require(row.get("record") == "case", f"Unknown record: {path}")
        key = (row["case"], row["allocator"])
        require(key in expected and key not in rows, f"Unknown/duplicate case: {path}/{key}")
        rows[key] = row
        require(row["mode"] == ("timed" if timed else "smoke"), f"Mixed modes: {path}/{key}")
        require(row["source_bytes"] > 0 and row["output_bytes"] > 0, f"Empty workload: {path}/{key}")
        for field in ("source_fnv1a64", "input_fnv1a64", "output_fnv1a64", "dictionary_fnv1a64"):
            value = row[field]
            require(len(value) == 16 and all(c in "0123456789abcdef" for c in value), f"Invalid fingerprint: {path}/{key}")
        survey = row["allocation_survey"]
        require(set(survey) == {"before", "cold", "reset", "final_drop"}, f"Survey phases: {path}/{key}")
        if key[1] == "global":
            require(all(value is None for value in survey.values()), f"Global counters must be null: {path}/{key}")
        else:
            for phase in ("before", "cold", "final_drop"):
                validate_snapshot(survey[phase], f"{path}/{key}/{phase}")
            before, cold, final = (survey[phase] for phase in ("before", "cold", "final_drop"))
            require(before["attempts"] == before["live_blocks"] == 0, f"Survey reused earlier work: {path}/{key}")
            require(before["identities_created"] == 1 and before["owners_dropped"] == 0, f"Survey owner setup: {path}/{key}")
            require(cold["attempts"] > 0, f"No payload request observed: {path}/{key}")
            require(final["live_blocks"] == final["live_bytes"] == final["live_requested_bytes"] == 0,
                    f"Unreleased payload: {path}/{key}")
            require(final["identities_created"] == final["owners_dropped"], f"Undropped owner: {path}/{key}")
            if key[1] == "tracking_borrowed":
                require(final["identities_created"] == 1, f"Borrowed owner cloned: {path}/{key}")
            if survey["reset"] is not None:
                validate_snapshot(survey["reset"], f"{path}/{key}/reset")
            phases = [survey[phase] for phase in ("before", "cold", "reset", "final_drop") if survey[phase] is not None]
            for prior, later in zip(phases, phases[1:]):
                for field in ("attempts", "requested_bytes", "peak_bytes", "peak_requested_bytes", "peak_live_blocks"):
                    require(later[field] >= prior[field], f"Cumulative counter decreased: {path}/{key}/{field}")
        if row["timing_scope"] == "warm_reset_process":
            require(row["cold_capacity"] is not None and row["reset_capacity"] is not None,
                    f"Missing retained context capacities: {path}/{key}")
            if key[1] != "global":
                require(survey["reset"] is not None, f"Missing reset survey: {path}/{key}")
        else:
            require(row["timing_scope"] == "fresh_allocate_drop" and row["reset_capacity"] is None and survey["reset"] is None,
                    f"Unexpected cold-operation reset: {path}/{key}")
        samples = row["samples"]
        if not timed:
            require(samples == [] and row["timing"] is None, f"Smoke produced timing data: {path}/{key}")
        else:
            require(len(samples) == metadata["samples"], f"Missing samples: {path}/{key}")
            values = []
            for sample in samples:
                require(sample["iterations"] > 0 and sample["elapsed_ns"] >= metadata["minimum_sample_ms"] * 1_000_000,
                        f"Invalid timed sample: {path}/{key}")
                values.append(sample["elapsed_ns"] / sample["iterations"] / row["source_bytes"])
            for field, expected_value in (("median_ns_per_byte", statistics.median(values)),
                                           ("minimum_ns_per_byte", min(values)), ("maximum_ns_per_byte", max(values))):
                require(math.isclose(row["timing"][field], expected_value, rel_tol=1e-12), f"Timing summary mismatch: {path}/{key}/{field}")
    require(set(rows) == expected, f"Missing cases: {path}: {expected - set(rows)}")
    for case in CASES:
        reference = rows[(case, "global")]
        for mode in MODES[1:]:
            for field in ("timing_scope", "unit", "input_bytes", "input_fnv1a64", "source_bytes", "source_fnv1a64", "dictionary_bytes", "dictionary_fnv1a64", "output_bytes", "output_fnv1a64"):
                require(rows[(case, mode)][field] == reference[field], f"Allocator workload/output differs: {path}/{case}/{mode}/{field}")
    return metadata, rows


def compare(reference, candidate, configuration):
    reference_metadata, reference_rows = reference
    metadata, rows = candidate
    require({key: value for key, value in metadata.items() if key != "features"} ==
            {key: value for key, value in reference_metadata.items() if key != "features"}, f"Metadata differs: {configuration}")
    for key in reference_rows:
        left = {field: value for field, value in reference_rows[key].items() if field not in ("samples", "timing")}
        right = {field: value for field, value in rows[key].items() if field not in ("samples", "timing")}
        require(left == right, f"Output, retained capacity, or allocator survey differs: {configuration}/{key}")


def write_report(output, configurations, results, timed):
    rows = results["safe"][1]
    lines = ["# Allocator benchmark " + ("measurements" if timed else "smoke validation"), "",
             f"All {len(CASES) * len(MODES)} workload/allocation-mode rows agree across {len(configurations)} backend configurations.",
             "Full output bytes are compared against each workload's Global reference before and after timing;",
             "source, dictionary and full-output FNV-1a fingerprints also match across configurations.",
             "These noncryptographic fingerprints supplement the codec's separate interoperability tests.", "",
             "Streaming rows time reset/process with reused context and fixed caller buffers. Whole-buffer,",
             "parsing and training rows time fresh operation allocation and result destruction. Full scans,",
             "fingerprints, correctness comparisons, allocator setup, source/dictionary preparation and I/O",
             "are outside timing. Progress/stall checks and black_box observation remain in streaming drivers.", "",
             "TrackingAllocator delegates storage to Global. Its timings include lock/ledger bookkeeping and",
             "fresh allocator-owner identities where applicable; this does not measure a different heap allocator.",
             "Owned tracking clones are deliberately non-equivalent domains. Borrowed tracking uses one non-Clone",
             "owner and reference copies. Global payload counters are uninstrumented null values, never inferred.", "",
             "Each allocation survey uses its own fresh monitor, independently of timing: before construction,",
             "after one cold operation with results/context live, after streaming reset, and after final owner drop.",
             "Peak values are cumulative high-water marks, not differences or per-operation peak estimates.",
             "Live/peak bytes describe returned payload layouts; they exclude underlying allocator rounding",
             "beyond returned sizes and transient Global old/new overlap during resizing. Separate retained",
             "capacities describe reachable context heap or result Vec capacity, excluding inline objects and",
             "caller buffers. Prepared streaming context capacity includes the external borrowed dictionary;",
             "its payload, any Arc/Rc control blocks, fixture setup, helper Arc/Mutex/ledger, and caller buffers",
             "are outside the context's measured allocator domain. Dictionary/training retained capacity is null",
             "because the public Dictionary API does not expose it; serialized length is not relabeled capacity.", ""]
    if not timed:
        lines += ["Smoke mode started no Rust Instant clocks and collected no throughput samples.", ""]
    else:
        lines += ["Values below are median ns per workload byte, followed by the observed sample range.",
                  "Codec units are uncompressed bytes; parsing uses serialized dictionary bytes; training uses sample bytes.",
                  "They include the stated harness/API costs, are host/configuration specific, and are not universal speedups.",
                  "All raw samples and all backend configurations remain in the adjacent JSONL/JSON records.", "",
                  "| Safe-backend workload | Global | Tracking owned | Tracking borrowed |",
                  "| --- | ---: | ---: | ---: |"]
        for case in CASES:
            cells = []
            for mode in MODES:
                value = rows[(case, mode)]["timing"]
                cells.append(f"{value['median_ns_per_byte']:.3f} [{value['minimum_ns_per_byte']:.3f}, {value['maximum_ns_per_byte']:.3f}]")
            lines.append("| " + " | ".join([case, *cells]) + " |")
        lines.append("")
    lines += ["| Workload | Allocator | Output bytes | Cold attempts | Cold live bytes | Cold peak bytes | Cold identities | Cold / reset capacity |",
              "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for case in CASES:
        for mode in MODES:
            row = rows[(case, mode)]
            metrics = row["allocation_survey"]["cold"]
            values = [str(metrics[key]) if metrics else "not instrumented" for key in ("attempts", "live_bytes", "peak_bytes", "identities_created")]
            capacities = " / ".join("not exposed" if row[key] is None else str(row[key]) for key in ("cold_capacity", "reset_capacity"))
            lines.append("| " + " | ".join([case, mode, str(row["output_bytes"]), *values, capacities]) + " |")
    lines += ["", "All tracked final-drop snapshots report zero live payload and no violations or failures.",
              "Allocation counts/capacities are checked for equality across backend builds on this host;",
              "they are not assumed invariant across pointer widths, operating systems, or allocators.", ""]
    (output / "summary.md").write_text("\n".join(lines))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", nargs="?", default="target/allocator-benchmarks")
    parser.add_argument("--timed", action="store_true", help="Explicitly collect timings; run only on a quiet host")
    parser.add_argument("--configs", default=",".join(("safe", *FEATURES, "all")))
    parser.add_argument("--toolchain", default=os.environ.get("ZSTDOXIDE_TOOLCHAIN", "nightly-2026-10-08"))
    parser.add_argument("--validate-only", action="store_true", help="Validate existing JSONL results without building or executing")
    args = parser.parse_args()
    configurations = args.configs.split(",")
    require(configurations[0] == "safe" and len(configurations) == len(set(configurations)) and
            set(configurations) <= {"safe", *FEATURES, "all"}, "Configurations must start with safe and contain no duplicates")
    declared = tomllib.loads((ROOT / "Cargo.toml").read_text())["features"]
    require(set(declared) - {"std", "default"} == set(FEATURES), "Update the allocator runner's independent unsafe-feature list")
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    original_sources = source_manifest()
    results = {}
    commands = []
    if not args.validate_only:
        (output / "source.json").write_text(json.dumps(original_sources, indent=2) + "\n")
        environment = {
            "timestamp_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "platform": platform.platform(), "machine": platform.machine(),
            "compiler": command_output(["rustc", "+" + args.toolchain, "-vV"]),
            "head": command_output(["git", "rev-parse", "HEAD"]),
            "status": command_output(["git", "status", "--short"]),
        }
        (output / "environment.json").write_text(json.dumps(environment, indent=2) + "\n")
    for configuration in configurations:
        path = output / (configuration + ".jsonl")
        if not args.validate_only:
            command = ["cargo", "+" + args.toolchain, "bench", "--offline", "--locked", "--bench", "allocators"]
            if configuration == "all":
                command.append("--all-features")
            elif configuration != "safe":
                command += ["--features", configuration]
            env = os.environ | {"ZSTDOXIDE_ALLOCATOR_BENCH_SMOKE": "0" if args.timed else "1"}
            print(f"Allocator {'measurement' if args.timed else 'smoke'}: {configuration}", flush=True)
            log = output / (configuration + ".cargo.log")
            with path.open("w") as stdout, log.open("w") as stderr:
                run = subprocess.run(command, cwd=ROOT, env=env, stdout=stdout, stderr=stderr, check=False)
            commands.append({"configuration": configuration, "command": command,
                             "environment": {name: env[name] for name in ("ZSTDOXIDE_ALLOCATOR_BENCH_SMOKE", "ZSTDOXIDE_ALLOCATOR_BENCH_SAMPLES", "ZSTDOXIDE_ALLOCATOR_BENCH_MS", "CARGO_TARGET_DIR") if name in env},
                             "exit_code": run.returncode, "stdout": path.name, "stderr": log.name})
            (output / "commands.json").write_text(json.dumps(commands, indent=2) + "\n")
            require(run.returncode == 0, f"Benchmark failed; see {log}")
            require(source_manifest() == original_sources, "Source or input changed during run; discard the campaign and restart from a frozen snapshot")
        results[configuration] = parse_results(path, configuration, args.timed)
        if configuration != "safe":
            compare(results["safe"], results[configuration], configuration)
    if not args.validate_only:
        (output / "results.json").write_text(json.dumps({
            "schema_version": 1, "mode": "timed" if args.timed else "smoke", "source_sha256": original_sources["sha256"],
            "configurations": {name: {"metadata": metadata, "rows": list(rows.values())} for name, (metadata, rows) in results.items()},
        }, indent=2) + "\n")
    write_report(output, configurations, results, args.timed)
    print(f"Verified {len(CASES) * len(MODES)} rows across {len(configurations)} configurations: {output / 'summary.md'}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, json.JSONDecodeError) as error:
        print(f"allocator benchmark: {error}", file=sys.stderr)
        sys.exit(1)
