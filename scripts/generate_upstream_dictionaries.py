#!/usr/bin/env python3
"""Generate byte-exact dictionary conformance probes with pinned zstd oracles.

Wire constants derive from facebook/zstd v1.5.7's format under BSD-3-Clause;
see LICENSE.zstd. Independent logical-source models are not C acceptance rules.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "tests/upstream/dictionaries"
ORACLE = ROOT / "scripts/oracles/dictionary_vectors.c"
DECODE_SOURCES = [
    "lib/common/debug.c", "lib/common/entropy_common.c", "lib/common/error_private.c",
    "lib/common/fse_decompress.c", "lib/common/xxhash.c", "lib/common/zstd_common.c",
    "lib/decompress/huf_decompress.c", "lib/decompress/zstd_ddict.c",
    "lib/decompress/zstd_decompress.c", "lib/decompress/zstd_decompress_block.c",
]
PROVENANCE = DECODE_SOURCES + [
    "lib/zstd.h", "lib/common/zstd_internal.h", "lib/compress/zstd_compress_internal.h",
    "doc/zstd_compression_format.md", "tests/fuzzer.c", "tests/zstreamtest.c",
    "tests/invalidDictionaries.c", "tests/fuzz/dictionary_loader.c",
    "tests/fuzz/dictionary_decompress.c", "tests/fuzz/dictionary_round_trip.c",
    "tests/fuzz/dictionary_stream_round_trip.c", "tests/fuzz/raw_dictionary_round_trip.c",
]
ML_BASE = list(range(3, 35)) + [
    35, 37, 39, 41, 43, 47, 51, 59, 67, 83, 99, 131, 259, 515, 1027, 2051,
    4099, 8195, 16387, 32771, 65539,
]
ML_BITS = [0] * 32 + [1, 1, 1, 1, 2, 2, 3, 3, 4, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def noise(size, state):
    result = bytearray()
    for _ in range(size):
        state ^= (state << 13) & 0xffffffff
        state ^= state >> 17
        state ^= (state << 5) & 0xffffffff
        result.append(state & 255)
    return bytes(result)


def block_header(size, kind, last):
    return ((size << 3) | (kind << 1) | last).to_bytes(3, "little")


def one_match(distance, length):
    """One sequence with no literals and RLE LL/OF/ML distributions."""
    offset_value = distance + 3
    offset_code = offset_value.bit_length() - 1
    match_code = max(i for i, base in enumerate(ML_BASE) if base <= length)
    match_bits = ML_BITS[match_code]
    bits = length - ML_BASE[match_code]
    bits |= (offset_value - (1 << offset_code)) << match_bits
    count = match_bits + offset_code
    bits |= 1 << count
    return bytes([0, 1, 0x54, 0, offset_code, match_code]) + bits.to_bytes((count + 8) // 8, "little")


def boundary_vectors():
    dictionary = noise(2048, 0x12345678)
    outputs = {"boundary-dictionary.raw": dictionary}
    records = [{"path": "boundary-dictionary.raw", "role": "raw_dictionary"}]
    cases = []
    for name, produced, distance, length in [
        ("p0-far-dictionary", 0, 2048, 3),
        ("p-window-minus-one", 1023, 1087, 3),
        ("p-window-equal", 1024, 1088, 3),
        ("p-window-plus-one", 1025, 1089, 3),
        ("dictionary-to-frame-eviction", 900, 1100, 300),
        ("output-distance-equals-window", 1025, 1024, 3),
        ("dictionary-out-of-bounds", 900, 2949, 3),
    ]:
        prefix = noise(produced, 0x9abcdef0)
        frame = bytearray(bytes.fromhex("28b52ffd0000"))
        for begin in range(0, produced, 1024):
            raw = prefix[begin:begin + 1024]
            frame += block_header(len(raw), 0, 0) + raw
        payload = one_match(distance, length)
        frame += block_header(len(payload), 2, 1) + payload
        expected = bytearray(prefix)
        in_bounds = distance <= produced + len(dictionary)
        if in_bounds:
            for _ in range(length):
                source = len(expected) - distance
                expected.append(dictionary[len(dictionary) + source] if source < 0 else expected[source])
        valid = in_bounds and (distance <= produced or produced <= 1024)
        outputs[name + ".zst"] = bytes(frame)
        outputs[name + ".expected"] = bytes(expected)
        records += [
            {"path": name + ".zst", "role": "boundary_frame",
             "dictionary": "boundary-dictionary.raw", "expected_output": name + ".expected",
             "rust_expectation": "accept" if valid else "reject"},
            {"path": name + ".expected", "role": "independent_logical_output"},
        ]
        cases.append({"name": name, "window": 1024, "produced_before_match": produced,
                      "distance": distance, "match_length": length, "source_in_bounds": in_bounds,
                      "rust_expectation": "accept" if valid else "reject"})
    expected = outputs["dictionary-to-frame-eviction.expected"]
    ring = bytearray(expected[:900])
    naive = bytearray(ring)
    for _ in range(300):
        source = len(naive) - 1100
        byte = dictionary[len(dictionary) + source] if source < 0 else ring[source % 1024]
        if len(ring) < 1024:
            ring.append(byte)
        else:
            ring[len(naive) % 1024] = byte
        naive.append(byte)
    mismatch = next(i for i, pair in enumerate(zip(naive, expected)) if pair[0] != pair[1])
    assert mismatch == 1100
    outputs["dictionary-to-frame-eviction.naive-ring"] = bytes(naive)
    records.append({"path": "dictionary-to-frame-eviction.naive-ring", "role": "incorrect_ring_model"})
    return outputs, records, cases, {"first_mismatch": mismatch, "expected_byte": expected[mismatch],
                                    "naive_byte": naive[mismatch]}


def compile_oracle(upstream, temporary, assertions):
    executable = temporary / ("dictionary-assertions" if assertions else "dictionary-vectors")
    flags = ["-std=c99", "-O1" if assertions else "-O2", "-Wall", "-Wextra", "-Werror",
             "-DZSTD_LEGACY_SUPPORT=0", "-DZSTD_DISABLE_ASM"]
    if assertions:
        flags += ["-DDEBUGLEVEL=1", "-DFUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION",
                  "-DFUZZING_ASSERT_VALID_SEQUENCE"]
    subprocess.run([os.environ.get("CC", "cc"), *flags, "-I", str(upstream / "lib"),
                    str(ORACLE), *(str(upstream / name) for name in DECODE_SOURCES),
                    "-o", str(executable)], check=True)
    return executable


def verify_boundaries(upstream, temporary, outputs, cases):
    for name, data in outputs.items():
        (temporary / name).write_bytes(data)
    # Assertion failures are deliberate evidence; never leave core dump files.
    try:
        import resource
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    except ImportError:
        pass
    results = []
    for assertions in [False, True]:
        executable = compile_oracle(upstream, temporary, assertions)
        for case in cases:
            methods = [("direct", 0), ("stream", 7)] if assertions else [
                ("direct", 0), ("prepared", 0), ("stream", 1), ("stream", 7), ("stream", 1024)]
            for method, chunk in methods:
                arguments = [str(executable), "probe", str(temporary / "boundary-dictionary.raw"),
                             str(temporary / (case["name"] + ".zst")),
                             str(temporary / (case["name"] + ".expected")), "raw", method, str(chunk)]
                result = subprocess.run(arguments, capture_output=True, text=True)
                row = {"case": case["name"], "assertions": assertions, "method": method, "output_chunk": chunk}
                if result.returncode:
                    assert assertions and "assert" in result.stderr.lower(), result.stderr
                    row["outcome"] = "assertion"
                else:
                    row.update(json.loads(result.stdout))
                    row["outcome"] = "accepted" if row["success"] else "rejected"
                    if row["success"]:
                        assert row["matches_expected"], row
                    if not assertions:
                        assert row["success"] == case["source_in_bounds"], row
                expected_assertion = case["name"] in [
                    "p-window-equal", "p-window-plus-one", "dictionary-out-of-bounds"]
                if assertions:
                    assert (row["outcome"] == "assertion") == expected_assertion, row
                results.append(row)
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    upstream = args.upstream.resolve()
    revision = subprocess.check_output(["git", "-C", str(upstream), "rev-parse", "HEAD"], text=True).strip()
    if revision != REVISION:
        parser.error(f"expected pinned revision {REVISION}, found {revision}")
    subprocess.run(["git", "-C", str(upstream), "diff", "--quiet", "HEAD", "--", "lib", "tests",
                    "doc/zstd_compression_format.md"], check=True)
    outputs, records, cases, eviction = boundary_vectors()
    with tempfile.TemporaryDirectory(prefix="zstdoxide-dictionaries-") as directory:
        results = verify_boundaries(upstream, Path(directory), outputs, cases)
    for record in records:
        data = outputs[record["path"]]
        record.update(size=len(data), sha256=digest(data))
    manifest = {
        "upstream_revision": REVISION,
        "generator": str(ORACLE.relative_to(ROOT)), "generator_sha256": digest(ORACLE.read_bytes()),
        "importer": str(Path(__file__).resolve().relative_to(ROOT)),
        "importer_sha256": digest(Path(__file__).read_bytes()),
        "source_sha256": {name: digest((upstream / name).read_bytes()) for name in PROVENANCE},
        "policy": "Dictionary references are accepted at P<=W and rejected at P>W; C production acceptance is recorded separately.",
        "fixtures": records, "boundary_cases": cases, "oracle_results": results,
        "eviction_evidence": eviction,
    }
    outputs["manifest.json"] = (json.dumps(manifest, indent=2) + "\n").encode()
    DESTINATION.mkdir(parents=True, exist_ok=True)
    for name, data in outputs.items():
        target = DESTINATION / name
        if args.check:
            if target.read_bytes() != data:
                raise RuntimeError(f"dictionary oracle reproducibility mismatch: {name}")
        else:
            target.write_bytes(data)
    print(f"{'Verified' if args.check else 'Generated'} {len(records)} dictionary fixtures and {len(results)} oracle outcomes")


if __name__ == "__main__":
    main()
