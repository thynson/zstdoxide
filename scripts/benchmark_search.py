#!/usr/bin/env python3
"""Search benchmark driver: clock-free smoke unless --timed is explicit.

Pins independent input identities and validates complete archived evidence.
Global contexts alone are timed. Owned TrackingAllocator surveys are separate.
"""
from __future__ import annotations
import argparse
import csv
import datetime
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import tomllib

FEATURES = (
    "unsafe-byte-cursor", "unsafe-bitstream", "unsafe-history", "unsafe-ring-history",
    "unsafe-fse", "unsafe-fse-encoder", "unsafe-huffman", "unsafe-huffman-codebook",
    "unsafe-match-table", "unsafe-match-chain", "unsafe-dictionary", "unsafe-training",
)
PROFILES = {
    "default": dict(zip(("hash_log", "chain_log", "key_bytes", "chain_visits", "lazy_visits", "history_bytes"), (16,18,4,8,16,131072))),
    "compact": dict(zip(("hash_log", "chain_log", "key_bytes", "chain_visits", "lazy_visits", "history_bytes"), (10,10,4,1,1,8192))),
    "wide": dict(zip(("hash_log", "chain_log", "key_bytes", "chain_visits", "lazy_visits", "history_bytes"), (18,19,8,32,64,1048576))),
}
WORKLOADS = ("block", "history", "dictionary")
POLICIES = ("greedy", "chain", "lazy")
EXPECTED = set(itertools.product(WORKLOADS, PROFILES, POLICIES))
FRAME_OPTIONS = {"window_log":20, "checksum":True, "pledged_size":None,
                 "include_content_size":True, "include_dictionary_id":True}
SNAPSHOT_FIELDS = {
    "allocate_calls", "allocate_zeroed_calls", "grow_calls", "grow_zeroed_calls", "shrink_calls",
    "deallocate_calls", "attempts", "successes", "failures", "zero_size_requests", "requested_bytes",
    "live_blocks", "peak_live_blocks", "live_requested_bytes", "peak_requested_bytes", "live_bytes",
    "peak_bytes", "identities_created", "owners_dropped", "violations",
}
ROW_FIELDS = {
    "record", "workload", "profile", "strategy", "mode", "timed_allocator", "timing_scope", "unit",
    "search", "frame_options", "input_chunk", "output_chunk", "block_count", "source_bytes", "source_fnv1a64",
    "dictionary_bytes", "dictionary_fnv1a64", "frame_bytes", "frame_fnv1a64", "compressed_size_ratio",
    "external_dictionary_capacity", "cold_search_capacity", "cold_total_capacity", "reused_search_capacity",
    "reused_total_capacity", "allocation_survey", "samples", "timing",
}
TIMING_FIELDS = {f"{band}_{unit}" for band in ("minimum", "median", "maximum")
                 for unit in ("ns_per_byte", "mib_per_second")}
FIXTURE = "tests/upstream/dictionaries/formatted.dict"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def exact(left, right):
    return json.dumps(left, sort_keys=True, allow_nan=False) == json.dumps(right, sort_keys=True, allow_nan=False)


def integer(value, minimum=0, maximum=None):
    return type(value) is int and value >= minimum and (maximum is None or value <= maximum)


def finite_positive(value):
    return type(value) in (int, float) and math.isfinite(value) and value > 0


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def fnv(data):
    result = 0xcbf29ce484222325
    for byte in data:
        result = ((result ^ byte) * 0x100000001b3) & ((1 << 64) - 1)
    return f"{result:016x}"


# This fixture boundary is independently recorded by the pinned v1.5.7 C
# dictionary generator (tests/upstream/dictionaries/manifest.json): 149 header
# bytes followed by 2048 content bytes. Rust's parser is not used to find it.
DICTIONARY_CONTENT_OFFSET = 149
DICTIONARY_CONTENT_BYTES = 2048
FIXTURE_SHA256 = "354d5499788e19c3b2bb4f0f85d1a733c12204c3333f0f22e7f4fb01067b3281"
INPUT_PINS = {'block': {'source_bytes': 32768, 'source_fnv1a64': '8e2f9283049eacd8', 'source_sha256': 'f62a7f5980b867d640cf8ddbbcd8f0a10b725ae4ee22d84d04c1a484ca71833b', 'dictionary_bytes': 0, 'dictionary_fnv1a64': 'cbf29ce484222325', 'dictionary_sha256': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'}, 'history': {'source_bytes': 393216, 'source_fnv1a64': 'a0f43a4eab5c6e2d', 'source_sha256': '668bee3ff532d0f9cca3ee528736ef44da22f46e448e15388e75e942cf065df8', 'dictionary_bytes': 0, 'dictionary_fnv1a64': 'cbf29ce484222325', 'dictionary_sha256': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'}, 'dictionary': {'source_bytes': 32768, 'source_fnv1a64': 'cc6272e6ae396202', 'source_sha256': '481bf8c0dbb2787163c7072476d7433f3afccc225f3253b7c94879158461d1f6', 'dictionary_bytes': 2197, 'dictionary_fnv1a64': 'd5e098c680a7348e', 'dictionary_sha256': '354d5499788e19c3b2bb4f0f85d1a733c12204c3333f0f22e7f4fb01067b3281'}}
BASE_ENV_KEYS = {
    "PATH", "HOME", "USER", "LOGNAME", "TMPDIR", "TMP", "TEMP", "SYSTEMROOT",
    "COMSPEC", "PATHEXT", "RUSTUP_HOME", "CARGO_HOME", "SDKROOT",
    "MACOSX_DEPLOYMENT_TARGET", "DEVELOPER_DIR",
}
FORCED_ENV = {
    "LC_ALL": "C", "LANG": "C", "TZ": "UTC", "CARGO_INCREMENTAL": "0",
    "CARGO_ENCODED_RUSTFLAGS": "", "RUSTFLAGS": "", "RUSTDOCFLAGS": "",
    "RUSTC": "rustc", "RUSTDOC": "rustdoc",
    "RUSTC_WRAPPER": "", "RUSTC_WORKSPACE_WRAPPER": "",
}
BUILD_RECORD_FIELDS = {"stage", "backend", "command", "cwd", "environment", "exit_code",
    "stdout", "stdout_sha256", "stderr", "stderr_sha256", "source_sha256"}
RUN_RECORD_FIELDS = BUILD_RECORD_FIELDS | {"executable_sha256"}
ENVIRONMENT_FIELDS = {"schema_version", "timestamp_utc", "platform", "machine", "toolchain",
    "host_target", "compiler", "head", "status", "configuration_order", "timed", "samples",
    "minimum_ms", "base_environment", "cargo_configs", "campaign_directory", "repository_root"}


def corpus_manifest(fixture):
    require(hashlib.sha256(fixture).hexdigest() == FIXTURE_SHA256, "Pinned dictionary fixture differs")
    require(len(fixture) == DICTIONARY_CONTENT_OFFSET + DICTIONARY_CONTENT_BYTES,
            "Pinned dictionary content boundary differs")
    mask = (1 << 64) - 1
    def next_byte(state):
        state ^= (state << 13) & mask
        state ^= state >> 7
        state ^= (state << 17) & mask
        return state, state & 255
    seed = 0x5eed5ea2c1202026
    state = seed
    text = b"configured-zstandard-search-workload"
    block = bytearray()
    for index in range(32768):
        state, random = next_byte(state)
        block.append(block[index - 8192] if index >= 8192 and index % 509 != 0
                     else text[index % len(text)] if index % 11 < 8 else random)
    history_parts = []
    for state in (seed ^ 1, seed ^ 2):
        part = bytearray()
        for _ in range(131072):
            state, byte = next_byte(state)
            part.append(byte)
        history_parts.append(part)
    require(history_parts[0] != history_parts[1], "Independent history sources are identical")
    history = history_parts[0] + history_parts[1] + history_parts[0]
    content = fixture[DICTIONARY_CONTENT_OFFSET:]
    dictionary = bytearray()
    for segment in range(64):
        start = (segment * 137 + 29) % (len(content) - 512 + 1)
        dictionary.extend(content[start:start + 512])
    workloads = {}
    for name, source in (("block", block), ("history", history), ("dictionary", dictionary)):
        selected = fixture if name == "dictionary" else b""
        workloads[name] = {"source_bytes": len(source), "source_fnv1a64": fnv(source),
            "source_sha256": hashlib.sha256(source).hexdigest(), "dictionary_bytes": len(selected),
            "dictionary_fnv1a64": fnv(selected), "dictionary_sha256": hashlib.sha256(selected).hexdigest()}
    require(exact(workloads, INPUT_PINS), "Independent generated input pins differ")
    return {"schema_version": 1, "recipe": "xorshift64-and-contiguous-dictionary-runs-v1",
        "seed": "5eed5ea2c1202026", "dictionary_fixture": FIXTURE,
        "dictionary_fixture_sha256": FIXTURE_SHA256,
        "dictionary_content_offset": DICTIONARY_CONTENT_OFFSET,
        "dictionary_content_bytes": DICTIONARY_CONTENT_BYTES,
        "dictionary_boundary_reference": "tests/upstream/dictionaries/manifest.json",
        "upstream_revision": "f8745da6ff1ad1e7bab384bd1f9d742439278e99", "workloads": workloads}


def effective_environment():
    # A whitelist avoids leaking credentials and ignores ambient Cargo profile,
    # target, wrapper and rustc-bootstrap variables. Explicit flags below also
    # override corresponding Cargo config values. Host target is passed by CLI.
    return {name: value for name, value in os.environ.items() if name in BASE_ENV_KEYS} | FORCED_ENV


def cargo_config_manifest(root, environment):
    # Cargo reads ancestor .cargo files and CARGO_HOME config. Hash these inputs
    # (never print their potentially private contents), including absence through
    # the exact path set, and recheck before/after every build and run.
    paths = {ancestor / ".cargo" / name for ancestor in (root, *root.parents)
             for name in ("config", "config.toml")}
    cargo_home = Path(environment.get("CARGO_HOME", str(Path(environment["HOME"]) / ".cargo"))).resolve()
    paths.update(cargo_home / name for name in ("config", "config.toml"))
    return [{"path": str(path), "bytes": path.stat().st_size, "sha256": sha(path)}
            for path in sorted(paths) if path.is_file()]


def build_command(root, campaign, configuration, toolchain, host_target, base_environment):
    target = campaign / "targets" / configuration
    command = ["cargo", "+" + toolchain, "bench", "--offline", "--locked", "--bench", "search",
               "--no-run", "--message-format=json", "--target", host_target]
    if configuration == "all":
        command.append("--all-features")
    elif configuration != "safe":
        command += ["--features", configuration]
    environment = base_environment | {"CARGO_TARGET_DIR": str(target), "CARGO_TERM_COLOR": "never"}
    return command, environment, target


def run_environment(environment):
    return environment["base_environment"] | {
        "ZSTDOXIDE_SEARCH_BENCH_TIMED": "1" if environment["timed"] else "0",
        "ZSTDOXIDE_SEARCH_BENCH_SAMPLES": str(environment["samples"]),
        "ZSTDOXIDE_SEARCH_BENCH_MS": str(environment["minimum_ms"])}


def executable_from_artifacts(path, target):
    executables = [record["executable"] for record in load_json_lines(path)
        if record.get("reason") == "compiler-artifact" and record.get("target", {}).get("name") == "search"
        and "bench" in record.get("target", {}).get("kind", []) and record.get("executable")]
    require(len(executables) == 1, "Require exactly one matching benchmark executable")
    require(type(executables[0]) is str, "Executable path type")
    executable = Path(executables[0])
    require(executable.is_absolute() and executable == executable.resolve()
            and executable.is_relative_to(target), "Unexpected benchmark executable path")
    return executable


def validate_environment(value, root, configurations, toolchain, timed, samples, minimum_ms):
    require(type(value) is dict and set(value) == ENVIRONMENT_FIELDS, "Environment evidence schema")
    require(exact(value["schema_version"], 1) and value["toolchain"] == toolchain
            and value["repository_root"] == str(root), "Environment source/toolchain identity")
    for name in ("timestamp_utc", "platform", "machine", "host_target", "campaign_directory"):
        require(type(value[name]) is str and bool(value[name]), f"Environment scalar: {name}")
    campaign = Path(value["campaign_directory"])
    require(campaign.is_absolute() and campaign == campaign.resolve(), "Campaign path is not canonical")
    require(exact(value["configuration_order"], configurations) and value["timed"] is timed
            and exact(value["samples"], samples) and exact(value["minimum_ms"], minimum_ms),
            "Environment configuration/mode/sample evidence differs")
    base = value["base_environment"]
    require(type(base) is dict and set(base) <= BASE_ENV_KEYS | set(FORCED_ENV)
            and all(type(item) is str for item in base.values())
            and all(base.get(name) == expected for name, expected in FORCED_ENV.items())
            and bool(base.get("PATH")) and bool(base.get("HOME")), "Uncontrolled effective environment")
    for name, command in (("compiler", ["rustc", "+" + toolchain, "-vV"]),
                          ("head", ["git", "rev-parse", "HEAD"]),
                          ("status", ["git", "status", "--short"])):
        record = value[name]
        require(type(record) is dict and set(record) == {"command", "exit_code", "stdout", "stderr"}
                and exact(record["command"], command) and exact(record["exit_code"], 0)
                and type(record["stdout"]) is str and type(record["stderr"]) is str,
                f"Environment probe evidence: {name}")
    hosts = [line.removeprefix("host: ") for line in value["compiler"]["stdout"].splitlines()
             if line.startswith("host: ")]
    require(hosts == [value["host_target"]], "Compiler host/explicit target differs")
    require(exact(value["cargo_configs"], cargo_config_manifest(root, base)), "Cargo configuration inputs differ")
    return value


def validate_command_records(recorded, root, output, source, environment):
    configurations = environment["configuration_order"]
    require(type(recorded) is list and all(type(record) is dict for record in recorded), "Command record type")
    expected_stages = [(configuration, stage) for configuration in configurations for stage in ("build", "run")]
    require([(record.get("backend"), record.get("stage")) for record in recorded] == expected_stages,
            "Require ordered successful build and run records for every backend")
    results = {}
    for configuration in configurations:
        build = recorded[2 * configurations.index(configuration)]
        run = recorded[2 * configurations.index(configuration) + 1]
        command, build_env, target = build_command(root, Path(environment["campaign_directory"]),
            configuration, environment["toolchain"], environment["host_target"], environment["base_environment"])
        for record, fields, stdout, stderr in (
            (build, BUILD_RECORD_FIELDS, configuration + ".build.jsonl", configuration + ".build.log"),
            (run, RUN_RECORD_FIELDS, configuration + ".jsonl", configuration + ".run.log")):
            require(set(record) == fields, "Command evidence schema")
            require(exact(record["exit_code"], 0) and record["cwd"] == str(root)
                    and record["source_sha256"] == source["sha256"], "Stored command/source evidence differs")
            require(record["stdout"] == stdout and record["stderr"] == stderr,
                    "Command artifact is detached from its canonical result")
            for stream in ("stdout", "stderr"):
                path = (output / record[stream]).resolve()
                require(path.is_relative_to(output) and path.is_file() and sha(path) == record[stream + "_sha256"],
                        "Stored command log hash differs")
        require(exact(build["command"], command) and exact(build["environment"], build_env),
                "Build command/effective environment differs")
        executable = executable_from_artifacts(output / build["stdout"], target)
        require(exact(run["command"], [str(executable)]) and exact(run["environment"], run_environment(environment)),
                "Run command/effective environment differs")
        require(type(run["executable_sha256"]) is str and len(run["executable_sha256"]) == 64
                and all(char in "0123456789abcdef" for char in run["executable_sha256"]), "Executable digest schema")
        # Large target directories need not be archived. If the original binary
        # remains available, additionally verify it against the run-time hash.
        if executable.is_file():
            require(sha(executable) == run["executable_sha256"], "Recorded executable bytes differ")
        results[configuration] = output / run["stdout"]
    return results

def source_manifest(root):
    paths = set(root.glob("src/**/*.rs")) | set((root / "test-support/allocator/src").glob("**/*.rs"))
    paths.update(root / name for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
        "benches/search.rs", "scripts/benchmark_search.py", "test-support/allocator/Cargo.toml", "tests/upstream/dictionaries/manifest.json", FIXTURE))
    digest = hashlib.sha256()
    files = []
    for path in sorted(paths):
        name = path.relative_to(root).as_posix()
        data = path.read_bytes()
        digest.update(name.encode() + b"\0" + data + b"\0")
        files.append({"path":name, "bytes":len(data), "sha256":hashlib.sha256(data).hexdigest()})
    return {"sha256":digest.hexdigest(), "files":files}


def snapshot(value, label):
    require(type(value) is dict and set(value) == SNAPSHOT_FIELDS, f"Snapshot schema: {label}")
    require(value["violations"] is False, f"Owner violation: {label}")
    require(all(integer(value[name]) for name in SNAPSHOT_FIELDS - {"violations"}), f"Counter type: {label}")
    require(value["failures"] == 0, f"Allocation failure: {label}")
    require(value["owners_dropped"] <= value["identities_created"], f"Owner totals: {label}")
    for current, peak in (("live_blocks", "peak_live_blocks"), ("live_bytes", "peak_bytes"),
                          ("live_requested_bytes", "peak_requested_bytes")):
        require(value[current] <= value[peak], f"Invalid peak: {label}/{peak}")


def parse_json(text):
    def bad_constant(value):
        raise ValueError(f"Non-JSON constant {value}")
    def unique_fields(pairs):
        result = {}
        for name, value in pairs:
            require(name not in result, f"Duplicate JSON field: {name}")
            result[name] = value
        return result
    return json.loads(text, parse_constant=bad_constant, object_pairs_hook=unique_fields)


def load_json(path):
    return parse_json(path.read_text())


def load_json_lines(path):
    return [parse_json(line) for line in path.read_text().splitlines() if line.strip()]


def parse_results(path, configuration, timed, inputs, samples=None, minimum_ms=None):
    records = load_json_lines(path)
    require(all(type(record) is dict for record in records), f"Record type: {path}")
    metadata = [row for row in records if row.get("record") == "metadata"]
    require(len(metadata) == 1, f"Expected one metadata record: {path}")
    metadata = metadata[0]
    expected_features = ["safe"] if configuration == "safe" else list(FEATURES) if configuration == "all" else [configuration]
    fixed = {"record":"metadata", "schema_version":1, "seed":"5eed5ea2c1202026", "ledger_slots":512,
             "small_bytes":32768, "history_block_bytes":131072, "dictionary_segment_bytes":512,
             "dictionary_start_stride":137, "dictionary_start_offset":29, "expected_rows":27,
             "features":expected_features}
    require(set(metadata) == set(fixed) | {"samples", "minimum_sample_ms"}, f"Metadata keys: {path}")
    require(exact({name:metadata[name] for name in fixed}, fixed), f"Metadata/scalars/features differ: {path}")
    require(integer(metadata["samples"],5,50) and integer(metadata["minimum_sample_ms"],1,60000), f"Sample bounds: {path}")
    if samples is not None:
        require(metadata["samples"] == samples, f"Requested sample count differs: {path}")
    if minimum_ms is not None:
        require(metadata["minimum_sample_ms"] == minimum_ms, f"Requested duration differs: {path}")
    rows = {}
    for row in records:
        if row.get("record") == "metadata":
            continue
        require(set(row) == ROW_FIELDS and row.get("record") == "case", f"Case schema: {path}")
        key = (row["workload"], row["profile"], row["strategy"])
        label = f"{path}/{key}"
        require(key in EXPECTED and key not in rows, f"Unknown/duplicate row: {label}")
        rows[key] = row
        require(row["mode"] == ("timed" if timed else "smoke"), f"Mixed mode: {label}")
        require(row["timed_allocator"] == "global" and row["timing_scope"] == "warm_reset_seed_rebuild_process"
                and row["unit"] == "uncompressed_byte", f"Timing domain: {label}")
        require(exact(row["search"],PROFILES[key[1]]) and exact(row["frame_options"],FRAME_OPTIONS), f"Complete options: {label}")
        source_size, block_count = (393216,3) if key[0] == "history" else (32768,1)
        for name, expected_value in (("source_bytes",source_size), ("block_count",block_count),
                                      ("input_chunk",4096), ("output_chunk",1024)):
            require(integer(row[name]) and row[name] == expected_value, f"Schedule/scalar mismatch: {label}/{name}")
        require(integer(row["frame_bytes"],1), f"Empty frame: {label}")
        for name in ("source_fnv1a64", "dictionary_fnv1a64", "frame_fnv1a64"):
            value = row[name]
            require(type(value) is str and len(value) == 16 and all(char in "0123456789abcdef" for char in value), f"Fingerprint: {label}/{name}")
        identity = inputs["workloads"][key[0]]
        require(integer(row["dictionary_bytes"]), f"Dictionary size type: {label}")
        require(all(exact(row[name], identity[name]) for name in (
            "source_bytes", "source_fnv1a64", "dictionary_bytes", "dictionary_fnv1a64")),
            f"Independent source/dictionary identity: {label}")
        ratio = row["compressed_size_ratio"]
        require(finite_positive(ratio) and math.isclose(ratio,row["frame_bytes"]/source_size,rel_tol=1e-12), f"Ratio: {label}")
        for name in ("external_dictionary_capacity", "cold_search_capacity", "cold_total_capacity", "reused_search_capacity", "reused_total_capacity"):
            require(integer(row[name]), f"Capacity type: {label}/{name}")
        external = row["external_dictionary_capacity"]
        require((external > 0) == (key[0] == "dictionary"), f"External dictionary scope: {label}")
        survey = row["allocation_survey"]
        require(type(survey) is dict and set(survey) == {"allocator","before","cold","reused","final_drop"}
                and survey["allocator"] == "tracking_owned_global_backed", f"Survey phases/domain: {label}")
        phases = [survey[phase] for phase in ("before","cold","reused","final_drop")]
        for phase, value in zip(("before","cold","reused","final_drop"),phases):
            snapshot(value,f"{label}/{phase}")
        before, cold, reused, final = phases
        require(before["attempts"] == before["live_blocks"] == before["live_bytes"] == before["live_requested_bytes"] == 0
                and before["identities_created"] == 1 and before["owners_dropped"] == 0, f"Fresh monitor: {label}")
        require(cold["attempts"] > 0, f"Empty allocation survey: {label}")
        for phase, value in (("cold",cold),("reused",reused)):
            require(0 < row[phase+"_search_capacity"] <= value["live_bytes"], f"Search subset: {label}/{phase}")
            require(value["live_bytes"] + external == row[phase+"_total_capacity"], f"External-domain accounting: {label}/{phase}")
        require(final["live_blocks"] == final["live_bytes"] == final["live_requested_bytes"] == 0
                and final["identities_created"] == final["owners_dropped"], f"Incomplete release: {label}")
        cumulative = SNAPSHOT_FIELDS - {"violations", "live_blocks", "live_requested_bytes", "live_bytes"}
        for prior, later in zip(phases, phases[1:]):
            require(all(later[name] >= prior[name] for name in cumulative), f"Counter decreased: {label}")
        sample_rows = row["samples"]
        require(type(sample_rows) is list, f"Sample type: {label}")
        if not timed:
            require(sample_rows == [] and row["timing"] is None, f"Smoke started timing: {label}")
        else:
            require(len(sample_rows) == metadata["samples"], f"Incomplete samples: {label}")
            values = []
            for sample in sample_rows:
                require(type(sample) is dict and set(sample) == {"iterations","elapsed_ns"}, f"Sample schema: {label}")
                require(integer(sample["iterations"],1,(1<<64)-1)
                        and integer(sample["elapsed_ns"],metadata["minimum_sample_ms"]*1_000_000,(1<<128)-1), f"Sample bound: {label}")
                value = sample["elapsed_ns"] / sample["iterations"] / source_size
                require(finite_positive(value), f"Sample value: {label}")
                values.append(value)
            expected_timing = dict(zip(("minimum_ns_per_byte","median_ns_per_byte","maximum_ns_per_byte"),
                                       (min(values),statistics.median(values),max(values))))
            throughputs = [1e9 / (value * 1048576) for value in values]
            expected_timing.update(dict(zip(("minimum_mib_per_second", "median_mib_per_second", "maximum_mib_per_second"),
                (min(throughputs), statistics.median(throughputs), max(throughputs)))))
            timing = row["timing"]
            require(type(timing) is dict and set(timing) == TIMING_FIELDS, f"Timing schema: {label}")
            require(all(finite_positive(timing[name]) and math.isclose(timing[name],value,rel_tol=1e-12)
                        for name,value in expected_timing.items()), f"Timing summary: {label}")
    require(set(rows) == EXPECTED, f"Incomplete 27-row matrix: {path}: {EXPECTED-set(rows)}")
    for workload in WORKLOADS:
        observed = {(row["source_bytes"],row["source_fnv1a64"],row["dictionary_bytes"],row["dictionary_fnv1a64"])
                    for key,row in rows.items() if key[0] == workload}
        require(len(observed) == 1, f"Corpus differs across profiles/policies: {path}/{workload}")
    return metadata, rows


def compare(reference, candidate, configuration):
    metadata, rows = candidate
    old_metadata, old_rows = reference
    require(exact({k:v for k,v in metadata.items() if k != "features"},
                  {k:v for k,v in old_metadata.items() if k != "features"}), f"Metadata differs: {configuration}")
    for key in EXPECTED:
        require(exact({k:v for k,v in rows[key].items() if k not in ("samples","timing")},
                      {k:v for k,v in old_rows[key].items() if k not in ("samples","timing")}),
                f"Non-timing output/scalar/capacity/allocation record differs: {configuration}/{key}")


def command_info(command, root, environment):
    result = subprocess.run(command,cwd=root,env=environment,text=True,capture_output=True,check=False)
    return {"command":command,"exit_code":result.returncode,"stdout":result.stdout,"stderr":result.stderr}


def write_report(output, results, timed):
    safe = results["safe"][1]
    lines = ["# Search benchmark " + ("measurements" if timed else "smoke validation"), "",
        f"All 27 rows agree on every non-timing record across {len(results)} declared backend configurations.",
        "Every row compares full fresh/reused Global output and independently decodes the complete source.",
        "Timed rows use only Global, including reset, dictionary seeding, history rebuilding and process/drain control.",
        "After each elapsed batch ends, its final output is compared with the verified reference.",
        "Caller buffers, sources, parsed dictionaries, checks, fingerprints and owned-tracker surveys stay outside clocks.",
        "Cross-process FNV64 fingerprints are noncryptographic supplemental evidence, not a C throughput comparison.",
        "Individual unsafe-feature builds may affect only untimed decoding or paths absent from these encoder workloads.",
        "Do not attribute every measured difference to an active optimized encoder accessor; existing backend microbench evidence is separate.", "",
        "Profiles are complete reproducible experimental controls, not recommended user presets or universal speedups.",
        "Configurations run in the recorded fixed order. Sample bands include host scheduling and thermal noise; they are not a significance test.",
        "Cost and throughput medians are computed independently from their individual sample values, including even sample counts.",
        "History uses two distinct 128-KiB sources as A/B/A under a 1-MiB wire window; dictionary input uses 64 contiguous 512-byte runs.",
        "TrackingAllocator wraps Global. Its separate Arc/Mutex/ledger bookkeeping and caller buffers are outside codec payload counts.",
        "Public total retained capacity includes the independently Global-owned dictionary; its measured clean-context contribution",
        "is recorded separately and checked against tracked live bytes. Search capacity is the narrower subset of owned storage.",
        "Peaks are observable live payload high-water marks, not Global's transient resize overlap or total process memory.", "",
        "| Workload | Profile | Policy | Frame bytes | Ratio | Search capacity | Total capacity | Cold / reused attempts | Median ns/B [min,max] |",
        "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for key in itertools.product(WORKLOADS,PROFILES,POLICIES):
        row = safe[key]
        timing = row["timing"]
        text = "no clocks" if timing is None else f"{timing['median_ns_per_byte']:.3f} [{timing['minimum_ns_per_byte']:.3f},{timing['maximum_ns_per_byte']:.3f}]"
        lines.append("| "+" | ".join([*key,str(row["frame_bytes"]),f"{row['compressed_size_ratio']:.6f}",
            str(row["reused_search_capacity"]),str(row["reused_total_capacity"]),
            f"{row['allocation_survey']['cold']['attempts']} / {row['allocation_survey']['reused']['attempts']}",text])+" |")
    lines += ["", "The table shows safe-backend rows. All configurations, raw samples, complete surveys and fingerprints remain in JSONL/results.json.",
              "backend-comparison.tsv preserves every backend's timing bands and row-specific safe/selected median ratio.",
              "A ratio above 1 means lower measured median cost for that row only. No aggregate or universal speedup is inferred.", ""]
    (output/"summary.md").write_text("\n".join(lines))
    with (output/"backend-comparison.tsv").open("w",newline="") as stream:
        writer = csv.writer(stream,delimiter="\t")
        writer.writerow(["backend","workload","profile","strategy",*PROFILES["default"],"frame_bytes","search_capacity",*sorted(TIMING_FIELDS),"safe_over_selected_median"])
        for configuration,(_,rows) in results.items():
            for key in itertools.product(WORKLOADS,PROFILES,POLICIES):
                row = rows[key]
                values = [row["timing"][name] for name in sorted(TIMING_FIELDS)] if timed else [""]*len(TIMING_FIELDS)
                ratio = safe[key]["timing"]["median_ns_per_byte"]/row["timing"]["median_ns_per_byte"] if timed else ""
                writer.writerow([configuration,*key,*[row["search"][name] for name in PROFILES["default"]],row["frame_bytes"],row["reused_search_capacity"],*values,ratio])


def result_document(source, timed, results):
    return {"schema_version": 1, "source_sha256": source["sha256"],
        "mode": "timed" if timed else "smoke",
        "configurations": {name: {"metadata": meta, "rows": list(rows.values())}
                           for name, (meta, rows) in results.items()}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", nargs="?", default="target/search-benchmarks")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--timed", action="store_true")
    parser.add_argument("--configs", default=",".join(("safe", *FEATURES, "all")))
    parser.add_argument("--toolchain", default="nightly-2026-10-08")
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--minimum-ms", type=int, default=100)
    parser.add_argument("--validate-only", action="store_true")
    args = parser.parse_args()
    root = args.root.resolve()
    output = Path(args.output).resolve()
    configurations = args.configs.split(",")
    require(configurations[0] == "safe" and len(configurations) == len(set(configurations))
            and set(configurations) <= {"safe", *FEATURES, "all"}, "Unique configurations must start with safe")
    require(5 <= args.samples <= 50 and 1 <= args.minimum_ms <= 60000, "Requested timing bounds")
    declared = tomllib.loads((root / "Cargo.toml").read_text())["features"]
    require(set(declared) - {"std", "default"} == set(FEATURES), "Update the independent unsafe-feature catalog")
    original = source_manifest(root)
    # The Python recipe and fixed pins are verified before any Rust process or
    # corpus-dependent timing; every backend must match these independent bytes.
    inputs = corpus_manifest((root / FIXTURE).read_bytes())
    output.mkdir(parents=True, exist_ok=True)
    results = {}
    commands = []
    if args.validate_only:
        require(exact(load_json(output / "source.json"), original), "Validation source/input manifest differs")
        require(exact(load_json(output / "inputs.json"), inputs), "Independent input manifest differs")
        environment = validate_environment(load_json(output / "environment.json"), root,
            configurations, args.toolchain, args.timed, args.samples, args.minimum_ms)
        commands = load_json(output / "commands.json")
        paths = validate_command_records(commands, root, output, original, environment)
    else:
        require(not (output / "source.json").exists(), "Preserve existing campaign; select a new output directory")
        (output / "source.json").write_text(json.dumps(original, indent=2) + "\n")
        (output / "inputs.json").write_text(json.dumps(inputs, indent=2) + "\n")
        base = effective_environment()
        require(bool(base.get("PATH")) and bool(base.get("HOME")), "PATH and HOME must be available")
        compiler = command_info(["rustc", "+" + args.toolchain, "-vV"], root, base)
        hosts = [line.removeprefix("host: ") for line in compiler["stdout"].splitlines() if line.startswith("host: ")]
        require(compiler["exit_code"] == 0 and len(hosts) == 1, "Unable to identify the selected compiler host")
        environment = {"schema_version": 1, "timestamp_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "platform": platform.platform(), "machine": platform.machine(), "toolchain": args.toolchain,
            "host_target": hosts[0], "compiler": compiler,
            "head": command_info(["git", "rev-parse", "HEAD"], root, base),
            "status": command_info(["git", "status", "--short"], root, base),
            "configuration_order": configurations, "timed": args.timed, "samples": args.samples,
            "minimum_ms": args.minimum_ms, "base_environment": base,
            "cargo_configs": cargo_config_manifest(root, base), "campaign_directory": str(output),
            "repository_root": str(root)}
        validate_environment(environment, root, configurations, args.toolchain, args.timed, args.samples, args.minimum_ms)
        (output / "environment.json").write_text(json.dumps(environment, indent=2) + "\n")
        reproduction = output / "reproduction"
        reproduction.mkdir()
        for relative in ("benches/search.rs", "scripts/benchmark_search.py"):
            shutil.copyfile(root / relative, reproduction / Path(relative).name)
        paths = {configuration: output / (configuration + ".jsonl") for configuration in configurations}
    def unchanged():
        require(exact(source_manifest(root), original), "Source/input changed during campaign")
        require(exact(cargo_config_manifest(root, environment["base_environment"]), environment["cargo_configs"]),
                "Cargo configuration inputs changed during campaign")
    for configuration in configurations:
        path = paths[configuration]
        if not args.validate_only:
            unchanged()
            build, env, target = build_command(root, output, configuration, args.toolchain,
                                               environment["host_target"], environment["base_environment"])
            artifacts = output / (configuration + ".build.jsonl")
            build_log = output / (configuration + ".build.log")
            with artifacts.open("w") as stdout, build_log.open("w") as stderr:
                process = subprocess.run(build, cwd=root, env=env, stdout=stdout, stderr=stderr, check=False)
            commands.append({"stage": "build", "backend": configuration, "command": build, "cwd": str(root),
                "environment": env, "exit_code": process.returncode,
                "stdout": artifacts.name, "stdout_sha256": sha(artifacts),
                "stderr": build_log.name, "stderr_sha256": sha(build_log), "source_sha256": original["sha256"]})
            (output / "commands.json").write_text(json.dumps(commands, indent=2) + "\n")
            require(process.returncode == 0, f"Build failed: {build_log}")
            unchanged()
            executable = executable_from_artifacts(artifacts, target)
            require(executable.is_file(), "Built benchmark executable is absent")
            executable_hash = sha(executable)
            run_env = run_environment(environment)
            print(f"Search {'timing' if args.timed else 'clock-free smoke'}: {configuration}", flush=True)
            unchanged()
            run_log = output / (configuration + ".run.log")
            with path.open("w") as stdout, run_log.open("w") as stderr:
                process = subprocess.run([str(executable)], cwd=root, env=run_env, stdout=stdout, stderr=stderr, check=False)
            commands.append({"stage": "run", "backend": configuration, "command": [str(executable)], "cwd": str(root),
                "environment": run_env, "executable_sha256": executable_hash, "exit_code": process.returncode,
                "stdout": path.name, "stdout_sha256": sha(path), "stderr": run_log.name,
                "stderr_sha256": sha(run_log), "source_sha256": original["sha256"]})
            (output / "commands.json").write_text(json.dumps(commands, indent=2) + "\n")
            require(process.returncode == 0, f"Run failed: {run_log}")
            require(sha(executable) == executable_hash, "Executable changed during run")
            unchanged()
        # Paths originate from validated canonical run stdout in replay mode;
        # fresh runs create that exact canonical path and validate all records below.
        results[configuration] = parse_results(path, configuration, args.timed, inputs, args.samples, args.minimum_ms)
        if configuration != "safe":
            compare(results["safe"], results[configuration], configuration)
    unchanged()
    validate_command_records(commands, root, output, original, environment)
    document = result_document(original, args.timed, results)
    if args.validate_only:
        require(exact(load_json(output / "results.json"), document), "Aggregated results/configurations differ")
    else:
        (output / "results.json").write_text(json.dumps(document, indent=2) + "\n")
    write_report(output, results, args.timed)
    print(f"Verified 27 rows across {len(results)} configurations: {output / 'summary.md'}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, TypeError, OverflowError, json.JSONDecodeError) as error:
        print(f"search benchmark: {error}", file=sys.stderr)
        sys.exit(1)
