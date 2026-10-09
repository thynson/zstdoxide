#!/usr/bin/env python3
"""Verify the bounded Fast smoke fixtures offline, or explicitly regenerate them."""
from pathlib import Path
import argparse
import datetime
import hashlib
import json
import os
import subprocess
import sys
import traceback
import shutil

ROOT = Path(__file__).resolve().parents[1]
COLLECTION = ROOT / "tests/upstream/fast-parser"
GENERATOR = "tests/upstream/fast-parser/fast_oracle.c"
IMPORTER = "scripts/import_fast_parser_smoke.py"
GENERATOR_SHA256 = "f9ab6e65ba20671560040d674d54ff64e78fc39c73a0e76f509722cb85b75f82"
OBSERVATIONS_SHA256 = "806bdbbadae4afa38d49390852955ec53b0b67c67d0e4e502b1c0517ad8ae5b7"
MAX_OBSERVATIONS_BYTES = 2 << 20
REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
RECIPES = {
    "short-seven": ("abcdefg", 0, 7, 10, 0, [1, 4, 8], False),
    "short-twelve": ("abcdefghijkl", 0, 12, 10, 0, [1, 4, 8], False),
    "repeat-one": ("A", 0, 64, 10, 0, [1, 4, 8], False),
    "repeat-four": ("abcd", 0, 64, 10, 0, [1, 4, 8], False),
    "offset-sixteen": ("0123456789ABCDEF", 0, 96, 10, 0, [1, 4, 8], False),
    "prefix-explicit": ("0123456789ABCDEF", 16, 64, 10, 0, [1, 4, 8], True),
    "prefix-duplicate-repeats": ("abcd", 16, 64, 10, 0, [4, 4, 8], True),
    "maximum-acceleration": ("abcd", 0, 64, 10, 131072, [1, 4, 8], False),
    "long-match": ("A", 0, 65700, 17, 0, [1, 4, 8], False),
}

def require(condition, reason):
    if not condition:
        raise ValueError(reason)

def exact(left, right):
    return json.dumps(left, sort_keys=True, allow_nan=False) == json.dumps(right, sort_keys=True, allow_nan=False)

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")

def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

def fingerprint(data):
    value = 0xcbf29ce484222325
    for byte in data:
        value = ((value ^ byte) * 0x100000001b3) & ((1 << 64) - 1)
    return f"{value:016x}"

def unique_json(line):
    def fields(pairs):
        result = {}
        for name, value in pairs:
            require(name not in result, f"duplicate JSON field {name}")
            result[name] = value
        return result
    def reject(value):
        raise ValueError(f"non-JSON scalar {value}")
    return json.loads(line, object_pairs_hook=fields, parse_constant=reject)

def validate(path):
    require(path.is_file() and path.stat().st_size <= MAX_OBSERVATIONS_BYTES, "bounded observations file")
    records = [unique_json(line) for line in path.read_text().splitlines()]
    require(len(records) == 19, "exactly one metadata and eighteen case records")
    metadata, rows = records[0], records[1:]
    require(type(metadata) is dict and set(metadata) == {
        "record", "schema_version", "upstream_revision", "entry_point", "observations",
        "strategy", "chain_log", "search_log", "initial_dict_limit", "initial_low_limit",
        "loaded_dict_end", "dict_match_state", "pointer_bits", "little_endian", "recipe_count",
        "case_count", "sizeof_seqdef", "sizeof_match_state"}, "exact metadata fields")
    require(metadata["schema_version"] == 1 and type(metadata["schema_version"]) is int
            and metadata["entry_point"] == "ZSTD_compressBlock_fast"
            and metadata["observations"] == "sequences-and-final-table-state-not-chronological-trace",
            "metadata version/entry point/scope")
    require(all(type(metadata[k]) is int for k in ("chain_log", "search_log", "initial_dict_limit",
            "initial_low_limit", "loaded_dict_end", "pointer_bits", "recipe_count", "case_count",
            "sizeof_seqdef", "sizeof_match_state"))
            and metadata["pointer_bits"] in (32, 64) and type(metadata["little_endian"]) is bool
            and metadata["sizeof_seqdef"] == 8 and 0 < metadata["sizeof_match_state"] <= 1 << 20,
            "metadata integer/host types")
    require(metadata["record"] == "metadata" and metadata["recipe_count"] == 9
            and metadata["case_count"] == 18 and metadata["upstream_revision"] == REVISION,
            "metadata count/revision")
    require(metadata["strategy"] == "fast" and metadata["chain_log"] == 6 and metadata["search_log"] == 1
            and metadata["initial_dict_limit"] == 2 and metadata["initial_low_limit"] == 2
            and metadata["loaded_dict_end"] == 0 and metadata["dict_match_state"] is False,
            "complete fixed parameter and no-dictionary state metadata")
    expected = {(name, padding) for name in RECIPES for padding in (0x6a, 0xc3)}
    observed = {}
    source_hashes = {}
    for row in rows:
        require(type(row) is dict and set(row) == {
            "record", "id", "padding", "pattern", "prefix_bytes", "block_bytes", "source_hex",
            "source_fnv1a64", "window_log", "hash_log", "min_match", "target_length",
            "physical_leading_bytes", "physical_suffix_bytes", "sequence_capacity", "literal_slack_bytes",
            "initial_repeats", "parser_repeats", "wire_repeats", "long_length_type",
            "long_length_position", "trailing_literals", "stored_literals_hex", "sequences",
            "table_before_c", "table_after_c", "table_before_positions", "table_after_positions",
            "table_before_le32_fnv1a64", "table_after_le32_fnv1a64", "replay_equal", "guards_intact"},
            "exact case fields")
        require(row["record"] == "case" and type(row["id"]) is str and type(row["padding"]) is int,
                "case identity types")
        key = row["id"], row["padding"]
        require(key in expected and key not in observed, "unexpected or duplicate recipe/padding")
        observed[key] = row
        pattern, prefix, block, window, target, initial, seeded = RECIPES[row["id"]]
        source = (pattern.encode() * ((prefix + block + len(pattern) - 1) // len(pattern)))[:prefix + block]
        require(type(row["source_hex"]) is str and len(row["source_hex"]) == 2 * len(source),
                "bounded complete source hexadecimal bytes")
        require(all(type(row[k]) is int and row[k] >= 0 for k in (
            "prefix_bytes", "block_bytes", "window_log", "hash_log", "min_match", "target_length",
            "physical_leading_bytes", "physical_suffix_bytes", "sequence_capacity",
            "literal_slack_bytes", "long_length_type", "long_length_position", "trailing_literals")),
            "case nonnegative integer fields")
        require(all(type(row[k]) is list and len(row[k]) == 3
                    and all(type(v) is int and 0 < v < 2**32 for v in row[k])
                    for k in ("initial_repeats", "parser_repeats", "wire_repeats")), "repeat array types")
        require(bytes.fromhex(row["source_hex"]) == source, "independent recipe bytes")
        require(row["source_fnv1a64"] == fingerprint(source), "source fingerprint")
        require(row["pattern"] == pattern and row["prefix_bytes"] == prefix and row["block_bytes"] == block
                and row["window_log"] == window and row["target_length"] == target
                and row["min_match"] == 4 and row["hash_log"] == 6
                and row["initial_repeats"] == initial, "independent recipe scalar metadata")
        require(row["physical_leading_bytes"] == 64 and row["physical_suffix_bytes"] == 262656
                and row["sequence_capacity"] == block // 3 + 1 and row["literal_slack_bytes"] == 32,
                "physical/capacity metadata")
        require(row["replay_equal"] is True and row["guards_intact"] is True, "helper success flags")
        require(type(row["stored_literals_hex"]) is str and len(row["stored_literals_hex"]) <= 2 * block,
                "bounded literal hexadecimal bytes")
        literals = bytes.fromhex(row["stored_literals_hex"])
        require(type(row["sequences"]) is list and len(row["sequences"]) <= block // 3 + 1,
                "bounded sequence count")
        replay = bytearray(source[:prefix])
        repeats = initial.copy()
        cursor = 0
        for sequence in row["sequences"]:
            require(type(sequence) is dict and set(sequence) == {"ll", "ml", "off_base", "distance", "intent"}, "sequence schema")
            ll, ml, off_base = sequence["ll"], sequence["ml"], sequence["off_base"]
            require(all(type(v) is int and v >= 0 for v in (ll, ml, off_base)), "sequence integers")
            require(ml >= 3 and 0 < off_base < 2**32 and type(sequence["distance"]) is int
                    and 0 < sequence["distance"] < 2**32, "sequence positive match/offset")
            require(cursor + ll <= len(literals), "literal source range")
            replay.extend(literals[cursor:cursor + ll]); cursor += ll
            if off_base > 3:
                distance = off_base - 3
                repeats = [distance, repeats[0], repeats[1]]
            else:
                selector = off_base - 1 + (ll == 0)
                if selector == 0:
                    distance = repeats[0]
                elif selector == 1:
                    distance = repeats[1]
                    repeats = [distance, repeats[0], repeats[2]]
                else:
                    distance = repeats[2] if selector == 2 else repeats[0] - 1
                    repeats = [distance, repeats[0], repeats[1]]
            require(sequence["distance"] == distance and distance > 0 and distance <= len(replay), "wire offset intent")
            require(sequence["intent"] == ("repeat" if off_base <= 3 else "offset"), "offset label")
            require(len(replay) + ml <= len(source), "bounded match reconstruction")
            for _ in range(ml):
                replay.append(replay[-distance])
        require(cursor == len(literals), "stored literals exhausted")
        require(row["trailing_literals"] == len(source) - len(replay), "last literals coverage")
        replay.extend(source[len(replay):])
        require(bytes(replay) == source, "independent Python byte replay")
        require(row["wire_repeats"] == repeats and row["parser_repeats"][:2] == repeats[:2]
                and row["parser_repeats"][2] == initial[2], "distinct repeat state")
        long_lengths = [(kind, index) for index, item in enumerate(row["sequences"])
                        for kind, length in ((1, item["ll"]), (2, item["ml"] - 3)) if length >= 65536]
        require(len(long_lengths) <= 1, "at most one long length per block")
        expected_long = long_lengths[0] if long_lengths else (0, 0)
        require((row["long_length_type"], row["long_length_position"]) == expected_long,
                "exact expanded long-length metadata")
        for phase in ("before", "after"):
            raw = row[f"table_{phase}_c"]
            require(type(raw) is list and len(raw) == 64 and all(type(v) is int and 0 <= v < 2**32 for v in raw), "table U32 schema")
            positions = row[f"table_{phase}_positions"]
            require(type(positions) is list and len(positions) == 64
                    and all(v is None or (type(v) is int and v >= 0) for v in positions),
                    "normalized table position types")
            require(row[f"table_{phase}_positions"] == [None if v < 2 else v - 2 for v in raw], "normalized table")
            require(row[f"table_{phase}_le32_fnv1a64"] == fingerprint(b"".join(v.to_bytes(4, "little") for v in raw)),
                    "canonical table fingerprint")
            for slot, value in enumerate(raw):
                require(value != 1, "sentinel one is not supplied or produced by Fast")
                if value:
                    require(2 <= value <= len(source) + 2 - 4, "complete table key fits logical source")
                    tail = source[value - 2:value - 2 + 4]
                    tail += bytes([row["padding"]]) * (4 - len(tail))
                    bucket = ((int.from_bytes(tail, "little") * 2654435761) & 0xffffffff) >> 26
                    require(bucket == slot, "independent hash bucket")
            if phase == "before":
                initial_table = [0] * 64
                if seeded:
                    bucket = ((int.from_bytes(source[:4], "little") * 2654435761) & 0xffffffff) >> 26
                    initial_table[bucket] = 2
                require(raw == initial_table, "exact prior-prefix seed")
        if row["id"] in ("short-seven", "short-twelve", "maximum-acceleration"):
            require(row["sequences"] == [] and row["trailing_literals"] == block, "short/large-step cleanup witness")
        if row["id"] == "prefix-explicit":
            require(row["sequences"] == [{"ll": 0, "ml": 64, "distance": 16, "off_base": 19, "intent": "offset"}],
                    "hand-readable explicit offset witness")
        if row["id"] == "long-match":
            require(row["long_length_type"] == 2 and row["long_length_position"] < len(row["sequences"])
                    and any(s["ml"] - 3 >= 65536 for s in row["sequences"]), "actual long-match metadata witness")
        source_hashes[row["id"]] = hashlib.sha256(source).hexdigest()
    require(set(observed) == expected, "complete recipes")
    require([(row["id"], row["padding"]) for row in rows] ==
            [(name, padding) for name in RECIPES for padding in (0x6a, 0xc3)],
            "canonical recipe/padding order")
    for name in RECIPES:
        left, right = observed[name, 0x6a], observed[name, 0xc3]
        require({k: v for k, v in left.items() if k != "padding"} ==
                {k: v for k, v in right.items() if k != "padding"},
                f"complete padding-pair observation independence: {name}")
    return {"recipes": 9, "cases": 18, "source_sha256": source_hashes, "metadata": metadata,
            "sequences": {name: observed[name, 0x6a]["sequences"] for name in RECIPES},
            "full_record_sha256": sha(path)}


def load_json(path, maximum=2 << 20):
    require(path.is_file() and path.stat().st_size <= maximum, f"bounded metadata: {path}")
    return unique_json(path.read_text())


def offline_check(collection):
    collection = collection.resolve()
    require(collection.is_dir(), "fixture collection is missing")
    expected_files = {"README.md", "NOTICE", "manifest.json", "fast_oracle.c", "observations.jsonl"}
    actual = {p.relative_to(collection).as_posix() for p in collection.rglob("*") if p.is_file()}
    require(actual == expected_files, "fixture collection contains missing/unlisted files")
    for name in expected_files:
        require((collection / name).resolve().is_relative_to(collection), "fixture path escapes collection")
    manifest = load_json(collection / "manifest.json")
    require(type(manifest) is dict and manifest.get("schema_version") == 1
            and type(manifest.get("schema_version")) is int
            and manifest.get("upstream_revision") == REVISION, "fixture schema/revision")
    require(exact(manifest.get("coverage"), {
        "kind": "bounded-direct-smoke", "recipes": 9, "observations": 18,
        "padding_patterns": [106, 195], "hash_log": 6, "effective_key_bytes": 4,
        "entry_point": "ZSTD_compressBlock_fast", "collector": False, "ordinary_frames": False}),
        "fixture coverage scope")
    require(manifest.get("generator") == GENERATOR and manifest.get("generator_sha256") == GENERATOR_SHA256
            and sha(collection / "fast_oracle.c") == GENERATOR_SHA256, "frozen oracle source bytes")
    require(manifest.get("importer") == IMPORTER and manifest.get("importer_sha256") == sha(Path(__file__)),
            "current importer source hash")
    observations = collection / "observations.jsonl"
    require(manifest.get("fixtures") == [{
        "path": "observations.jsonl", "size": observations.stat().st_size,
        "sha256": OBSERVATIONS_SHA256, "recipes": 9, "observations": 18}],
        "frozen observation manifest")
    require(sha(observations) == OBSERVATIONS_SHA256, "frozen C observation bytes")
    catalog = load_json(ROOT / "tests/upstream/manifest.json", 16 << 20)
    require(catalog.get("upstream", {}).get("revision") == REVISION, "repository source catalog revision")
    sources_by_path = {item["path"]: item["sha256"] for item in catalog["sources"] + catalog["excluded_sources"]}
    recorded = manifest.get("source_sha256")
    require(type(recorded) is dict and {
        "lib/zstd.h", "lib/compress/zstd_fast.h", "lib/compress/zstd_fast.c",
        "lib/compress/zstd_compress.c", "lib/compress/zstd_compress_internal.h",
        "lib/common/zstd_internal.h", "lib/common/mem.h"} <= set(recorded),
        "required reference sources")
    for name, digest in recorded.items():
        require(type(name) is str and sources_by_path.get(name) == digest, f"pinned source catalog: {name}")
    validation = validate(observations)
    require(exact(manifest["capture"]["metadata"], validation["metadata"]),
            "capture metadata differs from the observations")
    return manifest, validation


def environment():
    allowed = {"PATH", "HOME", "TMPDIR", "TMP", "TEMP", "SYSTEMROOT", "SDKROOT",
               "MACOSX_DEPLOYMENT_TARGET", "DEVELOPER_DIR"}
    return {k: v for k, v in os.environ.items() if k in allowed} | {
        "LC_ALL": "C", "LANG": "C", "TZ": "UTC"}


def regenerate(args, collection, manifest):
    upstream = args.upstream.resolve()
    output = args.output.resolve()
    report = args.report.resolve()
    require(not output.is_relative_to(ROOT) and not output.is_relative_to(collection),
            "regeneration output must be outside the repository/collection")
    require(not report.is_relative_to(ROOT) and not report.is_relative_to(collection),
            "regeneration report must be outside the repository/collection")
    require(not report.is_relative_to(output) and not output.is_relative_to(report),
            "regeneration output and report must have disjoint paths")
    require(not output.exists() and not report.exists(), "preserve existing output/report; choose fresh paths")
    output.mkdir(parents=True)
    report.parent.mkdir(parents=True, exist_ok=True)
    base = environment()
    compiler = shutil.which(args.cc, path=base.get("PATH"))
    require(compiler is not None, "C compiler was not found")
    compiler = str(Path(compiler).resolve())
    source_paths = [upstream / name for name in manifest["source_sha256"]]
    source_paths += [upstream / "lib/libzstd.a", collection / "fast_oracle.c", Path(__file__).resolve()]
    def source_state():
        return [{"path": str(p), "bytes": p.stat().st_size, "sha256": sha(p)} for p in source_paths]
    original = source_state()
    for name, digest in manifest["source_sha256"].items():
        require(sha(upstream / name) == digest, f"pinned upstream source differs: {name}")
    evidence = {"schema_version": 1, "scope": "bounded-direct-smoke", "started_at": now(),
                "upstream": str(upstream), "upstream_revision": REVISION,
                "collection": str(collection), "output": str(output), "environment": base,
                "compiler_executable": compiler, "compiler_sha256": sha(compiler),
                "source_before": original, "commands": [], "sanitize": args.sanitize}
    def save():
        write_json(report, evidence)
    def run(label, command, overrides=None):
        require(source_state() == original, "source/archive changed before command")
        stdout, stderr = output / f"{label}.stdout", output / f"{label}.stderr"
        env = base | (overrides or {})
        started = now()
        with stdout.open("w") as out, stderr.open("w") as err:
            process = subprocess.run(command, cwd=output, env=env, stdout=out, stderr=err,
                                     check=False, timeout=120)
        evidence["commands"].append({
            "label": label, "command": command, "cwd": str(output), "environment": env,
            "started_at": started, "finished_at": now(), "exit_code": process.returncode,
            "stdout": str(stdout), "stdout_sha256": sha(stdout),
            "stderr": str(stderr), "stderr_sha256": sha(stderr)})
        save()
        require(process.returncode == 0, f"command failed: {label}; diagnostics: {stderr}")
        require(source_state() == original, "source/archive changed after command")
        return stdout
    save()
    try:
        revision = run("reference-revision", ["git", "-C", str(upstream), "rev-parse", "HEAD"])
        require(revision.read_text().strip() == REVISION, "exact pinned revision required")
        status = run("reference-status", ["git", "-C", str(upstream), "status", "--porcelain"])
        require(status.read_text() == "", "pinned checkout has modified/untracked source")
        run("compiler-version", [compiler, "--version"])
        expected = [unique_json(line) for line in (collection / "observations.jsonl").read_text().splitlines()]
        generated = {}
        for mode in (("normal", "sanitized") if args.sanitize else ("normal",)):
            executable = output / ("fast_oracle-" + mode)
            command = [compiler, "-std=c11", "-Wall", "-Wextra", "-Werror",
                       "-I", str(upstream / "lib"), "-I", str(upstream / "lib/common"),
                       "-I", str(upstream / "lib/compress")]
            if mode == "sanitized":
                command += ["-O1", "-g", "-fsanitize=address,undefined", "-fno-omit-frame-pointer",
                            str(collection / "fast_oracle.c"), str(upstream / "lib/compress/zstd_fast.c")]
            else:
                command += ["-O2", str(collection / "fast_oracle.c")]
            command += [str(upstream / "lib/libzstd.a"), "-o", str(executable)]
            run(mode + "-compile", command)
            overrides = ({"ASAN_OPTIONS": "halt_on_error=1:detect_leaks=0",
                          "UBSAN_OPTIONS": "halt_on_error=1:print_stacktrace=1"} if mode == "sanitized" else {})
            records = run(mode + "-observations", [str(executable)], overrides)
            current = [unique_json(line) for line in records.read_text().splitlines()]
            validation = validate(records)
            require(current[1:] == expected[1:], f"{mode}: C case observations differ")
            # The host layout metadata is observed anew, not a false promise of
            # native execution on the original capture's architecture.
            host_fields = {"pointer_bits", "little_endian", "sizeof_match_state"}
            require({k: v for k, v in current[0].items() if k not in host_fields} ==
                    {k: v for k, v in expected[0].items() if k not in host_fields},
                    "stable metadata differs")
            generated[mode] = {"validation": validation, "executable_sha256": sha(executable),
                               "records_sha256": sha(records), "host_metadata": current[0],
                               "entire_capture_equal": records.read_bytes() == (collection / "observations.jsonl").read_bytes()}
        if args.sanitize:
            require((output / "normal-observations.stdout").read_bytes() ==
                    (output / "sanitized-observations.stdout").read_bytes(), "normal/sanitizer observations differ")
        evidence["results"] = generated
        evidence["source_after"] = source_state()
        require(evidence["source_after"] == original, "final source/archive changed")
        evidence["instrumentation"] = {
            "scope": "helper and unchanged zstd_fast.c plus inlined header code",
            "support_archive_instrumented": False, "leak_sanitizer": False}
        evidence["result"] = "passed"
        evidence["finished_at"] = now()
        save()
        print(f"Verified 18 observations / 9 recipes against pinned C ({', '.join(generated)}); report: {report}")
    except BaseException:
        evidence["failure"] = traceback.format_exc()
        save()
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="offline hashes and semantic checks; never invokes C")
    parser.add_argument("--collection", type=Path, default=COLLECTION, help="fixture collection (also permits isolated negative controls)")
    parser.add_argument("--upstream", type=Path, help="explicit clean built v1.5.7 checkout for regeneration")
    parser.add_argument("--output", type=Path, help="fresh external directory for regeneration artifacts")
    parser.add_argument("--report", type=Path, help="fresh external JSON report")
    parser.add_argument("--cc", default="cc", help="compiler executable, used only during explicit regeneration")
    parser.add_argument("--sanitize", action="store_true", help="also regenerate with ASan and UBSan")
    args = parser.parse_args()
    if args.check:
        require(args.upstream is None and args.output is None and args.report is None and not args.sanitize,
                "--check is offline and cannot be combined with regeneration options")
    else:
        require(args.upstream is not None and args.output is not None and args.report is not None,
                "use --check offline, or supply --upstream, --output and --report explicitly")
    collection = args.collection.resolve()
    manifest, validation = offline_check(collection)
    if args.check:
        print(f"Fast smoke fixtures valid: {validation['cases']} observations / {validation['recipes']} recipes; no C checkout/compiler required.")
    else:
        regenerate(args, collection, manifest)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, TypeError, OverflowError, subprocess.SubprocessError) as error:
        print(f"fast parser smoke: {error}", file=sys.stderr)
        raise SystemExit(1)
