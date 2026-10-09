#!/usr/bin/env python3
"""Capture/check pinned unsalted hash observations, never a codec binding.

--check compares all deterministic bytes and stable provenance without rewriting
capture metadata. --report records the current environment outside the collection.
--sanitize additionally requires ASan/UBSan observations to match the normal run.
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
import shutil
import subprocess
import tempfile
import time

REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "tests/upstream/match-search"
ORACLE = DESTINATION / "hash_oracle.c"
SOURCES = (
    "lib/zstd.h", "lib/zstd_errors.h", "lib/common/allocations.h",
    "lib/common/bits.h", "lib/common/bitstream.h", "lib/common/compiler.h",
    "lib/common/cpu.h", "lib/common/debug.h", "lib/common/error_private.h",
    "lib/common/fse.h", "lib/common/huf.h", "lib/common/mem.h",
    "lib/common/portability_macros.h", "lib/common/xxhash.h",
    "lib/common/zstd_deps.h", "lib/common/zstd_internal.h",
    "lib/common/zstd_trace.h", "lib/compress/zstd_compress_internal.h",
    "lib/compress/zstd_cwksp.h", "lib/compress/zstd_preSplit.h",
)
COMPATIBILITY_REFERENCE = {
    "kind": "Historical arithmetic guard, not independent Rust execution",
    "repository_revision": "0111aef8a862e36d8b4a812ee1c9a31032ee3fef",
    "source_sha256": {
        "src/matchfinder.rs": "8f5e28b2127533638947744917078c929a9e2da785b6c38fd7e6c13ada00f9a6",
        "src/matchchain.rs": "dbf5c8f65b8f6dedfaaa3907e22bb0e932029793bc0e1cbe412189113fc78e80",
    },
    "expression": "little_endian_u32.wrapping_mul(0x9e3779b1) >> 16",
    "policy": "Historical source attribution; these are not dependencies on mutable current Rust sources. The TSV current_rust_expression_bucket column means current at this recorded pre-Module-11 revision.",
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def table(data, fields):
    reader = csv.DictReader(io.StringIO(data.decode("ascii")), delimiter="\t")
    require(reader.fieldnames == fields, f"Unexpected TSV columns: {reader.fieldnames}")
    result = list(reader)
    require(result and all(None not in row and None not in row.values() for row in result),
            "Empty or malformed TSV records")
    return result


def patterns():
    values = [bytes.fromhex(value) for value in (
        "0000000000000000", "ffffffffffffffff", "0001020304050607", "0706050403020100",
        "0123456789abcdef", "efcdab8967452301", "55aa55aa55aa55aa", "512f8b0e76543210",
    )]
    names = ["zero", "ones", "ascending_bytes", "descending_bytes", "asymmetric",
             "asymmetric_reversed", "alternating", "existing_collision_word"]
    for byte in range(8):
        for bit in (0, 1, 6, 7):
            value = bytearray(8)
            value[byte] = 1 << bit
            values.append(bytes(value))
            names.append(f"single_bit_{8 * byte + bit}")
    state = 0x243f6a88
    for sample in range(24):
        value = bytearray()
        for _ in range(8):
            state ^= (state << 13) & 0xffffffff
            state ^= state >> 17
            state ^= (state << 5) & 0xffffffff
            state &= 0xffffffff
            value.append(state >> 24)
        values.append(bytes(value))
        names.append(f"xorshift32_{sample}")
    require(len(values) == len(set(values)) == 64, "Patterns must be 64 distinct records")
    text = "pattern\tfamily\tbytes\n" + "".join(
        f"{index}\t{name}\t{value.hex()}\n"
        for index, (name, value) in enumerate(zip(names, values, strict=True)))
    return values, {"patterns.bin": b"".join(values), "patterns.tsv": text.encode("ascii")}


def validate(values, outputs):
    rows = table(outputs["vectors.tsv"], ["width", "log", "pattern", "bytes", "bucket"])
    require(len(rows) == 2880, "Expected 2880 primitive rows")
    expected = [(width, log, pattern) for width in range(4, 9)
                for log in range(10, 19) for pattern in range(64)]
    buckets = {}
    for row, identity in zip(rows, expected, strict=True):
        key = tuple(int(row[field]) for field in ("width", "log", "pattern"))
        require(key == identity, "Primitive rows must retain complete deterministic order")
        width, log, pattern = key
        value = int(row["bucket"])
        require(0 <= value < 1 << log, "Bucket exceeds selected hash bits")
        require(row["bytes"] == values[pattern].hex(), "Primitive input differs from pattern")
        buckets[key] = value
    for width in range(4, 9):
        require(buckets[width, 16, 4] != buckets[width, 16, 5],
                "Asymmetric byte-order witness does not distinguish its reversal")
    pairs = table(outputs["invariance.tsv"], [
        "width", "log", "pattern", "base", "changed", "base_bucket",
        "changed_bucket", "full8_base", "full8_changed",
    ])
    require(len(pairs) == 2304, "Expected 2304 unused-high-byte pairs")
    expected = [(width, log, pattern) for width in range(4, 8)
                for log in range(10, 19) for pattern in range(64)]
    sensitivity = {str(width): 0 for width in range(4, 8)}
    for row, identity in zip(pairs, expected, strict=True):
        key = tuple(int(row[field]) for field in ("width", "log", "pattern"))
        require(key == identity, "Invariance rows must retain complete deterministic order")
        width, log, pattern = key
        base, changed = bytes.fromhex(row["base"]), bytes.fromhex(row["changed"])
        require(base == values[pattern] and len(changed) == 8, "Malformed pair inputs")
        require(base[:width] == changed[:width] and all(
            a ^ b == 255 for a, b in zip(base[width:], changed[width:], strict=True)),
            "Pair must preserve key bytes and flip every unused high byte")
        require(int(row["base_bucket"]) == int(row["changed_bucket"]) == buckets[key],
                "Unused high bytes changed a selected-width bucket")
        require(int(row["full8_base"]) == buckets[8, log, pattern],
                "Full-width base does not match primary oracle")
        require(0 <= int(row["full8_changed"]) < 1 << log, "Full-width bucket is out of bounds")
        sensitivity[str(width)] += row["full8_base"] != row["full8_changed"]
    require(all(count == 576 for count in sensitivity.values()),
            "Expected all captured width-eight sensitivity controls to differ")
    # These arithmetic guards describe the recorded prior Rust expression. The
    # C rows themselves always come from actual upstream pointer wrappers.
    compatibility = "pattern\tfirst4_le\tcurrent_rust_expression_bucket\tobserved_c_bucket\n"
    for pattern, value in enumerate(values):
        word = int.from_bytes(value[:4], "little")
        baseline = ((word * 0x9e3779b1) & 0xffffffff) >> 16
        observed = buckets[4, 16, pattern]
        require(baseline == observed, "Width-four historical expression changed")
        compatibility += f"{pattern}\t{word:08x}\t{baseline}\t{observed}\n"
    require(buckets[4, 16, 0] == buckets[4, 16, 7] == 0,
            "Existing zero/collision witness changed")
    require(buckets[4, 16, 8] == 0x9e37 and buckets[4, 16, 1] == 0x61c8,
            "Existing word-one/all-ones guards changed")
    outputs["compatibility.tsv"] = compatibility.encode("ascii")
    return {"patterns.bin": 64, "patterns.tsv": 64, "vectors.tsv": 2880,
            "invariance.tsv": 2304, "compatibility.tsv": 64}, sensitivity


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--sanitize", action="store_true", help="compare ASan/UBSan and optimized output")
    parser.add_argument("--report", type=Path, help="current execution metadata outside the fixture collection")
    args = parser.parse_args()
    if args.report and args.report.resolve().is_relative_to(DESTINATION.resolve()):
        parser.error("--report must be outside the fixture collection; capture must not be overwritten")
    upstream = args.upstream.resolve()
    records = []

    def run(command):
        start = time.monotonic()
        result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        records.append({"command": [str(part) for part in command],
                        "exit_code": result.returncode,
                        "seconds": round(time.monotonic() - start, 6),
                        "stdout_sha256": digest(result.stdout),
                        "stderr_sha256": digest(result.stderr),
                        "stderr": result.stderr.decode("utf-8", errors="replace")})
        if result.returncode:
            raise RuntimeError(f"Command failed: {shlex.join(map(str, command))}\n"
                               + result.stderr.decode("utf-8", errors="replace"))
        return result.stdout

    revision = run(["git", "-C", str(upstream), "rev-parse", "HEAD"]).decode().strip()
    require(revision == REVISION, f"Expected pinned revision {REVISION}, found {revision}")
    run(["git", "-C", str(upstream), "diff", "--quiet", "HEAD", "--"])
    source_hashes = {}
    for name in SOURCES:
        data = (upstream / name).read_bytes()
        require(data == run(["git", "-C", str(upstream), "show", REVISION + ":" + name]),
                f"Included source differs from pinned Git bytes: {name}")
        source_hashes[name] = digest(data)
    library = upstream / "lib/libzstd.a"
    require(library.is_file(), "Build the pinned external lib/libzstd.a before importing")
    archive_hash = digest(library.read_bytes())
    compiler = shlex.split(os.environ.get("CC", "cc"))
    require(bool(compiler), "CC must name a compiler")
    compiler_path = shutil.which(compiler[0])
    require(compiler_path is not None, "CC compiler executable not found")
    compiler_version = run([*compiler, "--version"]).decode()
    flags = ["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-Wno-unused-function"]
    sdk = None
    if platform.system() == "Darwin":
        sdk = run(["xcrun", "--show-sdk-path"]).decode().strip()
        flags += ["-isysroot", sdk]
    values, outputs = patterns()
    host = None
    included = set()
    with tempfile.TemporaryDirectory(prefix="zstdoxide-match-search-") as temporary:
        directory = Path(temporary)
        input_path = directory / "patterns.bin"
        input_path.write_bytes(outputs["patterns.bin"])
        for sanitized in ([False, True] if args.sanitize else [False]):
            name = "sanitized" if sanitized else "optimized"
            executable, depfile = directory / name, directory / (name + ".d")
            selected_flags = flags + (["-O1", "-fsanitize=address,undefined",
                                      "-fno-sanitize-recover=all", "-fno-omit-frame-pointer"]
                                     if sanitized else [])
            command = [*compiler, *selected_flags, "-I", str(upstream / "lib/compress"),
                       "-MMD", "-MF", str(depfile), str(ORACLE), str(library), "-o", str(executable)]
            run(command)
            current_host = table(run([str(executable), "--meta"]),
                                 ["version", "size_t_bits", "int_bits", "byte_order"])
            require(len(current_host) == 1, "Expected one host metadata row")
            current_host = current_host[0]
            require(current_host["version"] == "1.5.7"
                    and current_host["size_t_bits"] in {"32", "64"}
                    and current_host["int_bits"] == "32"
                    and current_host["byte_order"] in {"little", "big"},
                    f"Unsupported observed C host: {current_host}")
            if host is None:
                host = current_host
            else:
                require(host == current_host, "Sanitized and optimized host metadata differ")
            for section in ("vectors", "invariance"):
                data = run([str(executable), "--" + section, str(input_path)])
                filename = section + ".tsv"
                if sanitized:
                    require(outputs[filename] == data, f"Sanitized output differs: {section}")
                else:
                    outputs[filename] = data
            dependencies = shlex.split(depfile.read_text().replace("\\\n", " ").split(":", 1)[1])
            for dependency in dependencies:
                path = Path(dependency).resolve()
                if path.is_relative_to(upstream):
                    relative = path.relative_to(upstream).as_posix()
                    require(relative in source_hashes, f"Unrecorded upstream include: {relative}")
                    included.add(relative)
        require("lib/compress/zstd_compress_internal.h" in included,
                "Compiler did not include the pinned hash header")
    counts, sensitivity = validate(values, outputs)
    run(["git", "-C", str(upstream), "diff", "--quiet", "HEAD", "--"])
    require(archive_hash == digest(library.read_bytes()), "Reference archive changed during execution")
    require(source_hashes == {name: digest((upstream / name).read_bytes()) for name in SOURCES},
            "Pinned headers changed during execution")
    cli = upstream / "programs/zstd"
    cli_capture = None
    if cli.is_file():
        cli_capture = {"sha256": digest(cli.read_bytes()),
                       "version": run([str(cli), "--version"]).decode(),
                       "role": "Optional provenance only; never supplies hash observations"}
    capture = {
        "compiler_command": compiler, "compiler_executable": str(Path(compiler_path).resolve()),
        "compiler_executable_sha256": digest(Path(compiler_path).read_bytes()),
        "compiler_version": compiler_version, "sdk": sdk,
        "compile_command_template": [*compiler, *["<sdk>" if item == sdk else item for item in flags],
                                     "-I", "<upstream>/lib/compress", "-MMD", "-MF", "<temporary>/optimized.d",
                                     "<root>/tests/upstream/match-search/hash_oracle.c",
                                     "<upstream>/lib/libzstd.a", "-o", "<temporary>/optimized"],
        "host_system": platform.system(), "host_machine": platform.machine(), "observed_c_host": host,
        "static_archive_sha256": archive_hash, "cli": cli_capture,
        "compiler_included_sources": sorted(included),
        "sanitizer_output_equal": True if args.sanitize else None,
        "policy": "Capture compiler/archive/host are historical metadata; --check compares stable provenance/data and reports the current environment separately. Only the recorded host was executed; no inferred native platform coverage.",
    }
    stable = {
        "schema_version": 1, "upstream_revision": REVISION,
        "generator": ORACLE.relative_to(ROOT).as_posix(), "generator_sha256": digest(ORACLE.read_bytes()),
        "importer": Path(__file__).resolve().relative_to(ROOT).as_posix(),
        "importer_sha256": digest(Path(__file__).read_bytes()),
        "source_sha256": source_hashes,
        "compatibility_reference": COMPATIBILITY_REFERENCE,
        "fixtures": [{"path": name, "size": len(data), "sha256": digest(data),
                      **({"records": counts[name]} if name.endswith(".bin") else {"rows": counts[name]})}
                     for name, data in sorted(outputs.items())],
        "sensitivity_pairs_by_width": sensitivity,
        "policy": "C-only unsalted hash observations for widths 4..8/logs 10..18, with fully initialized eight-byte C inputs. Unused-high-byte pairs and historical four-byte arithmetic guards do not establish Rust execution, permit Rust overreads, or prove codec/candidate/strategy/ratio parity.",
    }
    manifest_path = DESTINATION / "manifest.json"
    if args.check:
        recorded = json.loads(manifest_path.read_text())
        require({key: value for key, value in recorded.items() if key != "capture"} == stable,
                "Stable hash provenance differs; intentional updates require regeneration")
        for name, data in outputs.items():
            require((DESTINATION / name).read_bytes() == data, f"Hash fixture bytes differ: {name}")
    else:
        DESTINATION.mkdir(parents=True, exist_ok=True)
        for name, data in outputs.items():
            (DESTINATION / name).write_bytes(data)
        manifest_path.write_text(json.dumps({**stable, "capture": capture}, indent=2) + "\n")
    report = {"mode": "check" if args.check else "generate", "revision": REVISION,
              "records": counts, "capture": capture, "commands": records,
              "stable_provenance_sha256": digest(json.dumps(stable, sort_keys=True).encode()),
              "fixtures": stable["fixtures"]}
    if args.report:
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"{'Verified' if args.check else 'Generated'} match-search fixtures: "
          + json.dumps(counts, sort_keys=True))
    print("Observed C host: " + json.dumps(host, sort_keys=True))
    print("Historical width-four arithmetic guard only; no Rust execution or codec parity claim.")


if __name__ == "__main__":
    main()
