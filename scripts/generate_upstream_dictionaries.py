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
ENCODE_SOURCES = [
    "lib/common/pool.c", "lib/common/threading.c",
    "lib/compress/fse_compress.c", "lib/compress/hist.c", "lib/compress/huf_compress.c",
    "lib/compress/zstd_compress.c", "lib/compress/zstd_compress_literals.c",
    "lib/compress/zstd_compress_sequences.c", "lib/compress/zstd_compress_superblock.c",
    "lib/compress/zstd_double_fast.c", "lib/compress/zstd_fast.c", "lib/compress/zstd_lazy.c",
    "lib/compress/zstd_ldm.c", "lib/compress/zstd_opt.c", "lib/compress/zstd_preSplit.c",
    "lib/compress/zstdmt_compress.c", "lib/dictBuilder/cover.c", "lib/dictBuilder/divsufsort.c",
    "lib/dictBuilder/fastcover.c", "lib/dictBuilder/zdict.c",
]
PROVENANCE = DECODE_SOURCES + ENCODE_SOURCES + [
    "lib/zdict.h", "lib/common/fse.h", "lib/common/huf.h",
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
        flags += ["-DDICTIONARY_PROBE_ONLY", "-DDEBUGLEVEL=1", "-DFUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION",
                  "-DFUZZING_ASSERT_VALID_SEQUENCE"]
    sources = DECODE_SOURCES if assertions else DECODE_SOURCES + ENCODE_SOURCES
    subprocess.run([os.environ.get("CC", "cc"), *flags, "-I", str(upstream / "lib"),
                    "-I", str(upstream / "lib/common"),
                    str(ORACLE), *(str(upstream / name) for name in sources),
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


def training_samples():
    """256 independently generated fixed-size records, including all byte values."""
    samples = []
    for index in range(256):
        record = (f'{{"service":"dictionary-fixtures","request":{index:04d},'
                  f'"route":"/api/items/{index % 17:02d}","status":200,"method":"GET",'
                  f'"user":"user-{index % 31:02d}","message":"deterministic training sample"}}\n').encode()
        random = noise(512, 0x9e3779b9 + index)
        body = bytearray((record * 4)[:512])
        for position in range(7, 512, 17):
            body[position] = random[position]
        body[0] = index
        samples.append(bytes(body))
    return b"".join(samples)


def first_block_modes(frame):
    """Independent wire inspection; do not infer entropy use from successful decoding."""
    descriptor = frame[4]
    single = (descriptor >> 5) & 1
    dictionary_bytes = [0, 1, 2, 4][descriptor & 3]
    content_bytes = [single, 2, 4, 8][descriptor >> 6]
    offset = 5 + (not single) + dictionary_bytes + content_bytes
    block_bits = int.from_bytes(frame[offset:offset + 3], "little")
    result = {"block_type": (block_bits >> 1) & 3, "last": bool(block_bits & 1),
              "block_size": block_bits >> 3}
    if result["block_type"] != 2:
        return result
    block = frame[offset + 3:offset + 3 + result["block_size"]]
    literal_kind = block[0] & 3
    size_format = (block[0] >> 2) & 3
    if literal_kind < 2:
        header_size = [1, 2, 1, 3][size_format]
        regenerated = int.from_bytes(block[:header_size], "little") >> (3 if header_size == 1 else 4)
        payload_size = 1 if literal_kind == 1 else regenerated
    else:
        header_size = [3, 3, 4, 5][size_format]
        bits = int.from_bytes(block[:header_size], "little")
        width = [10, 10, 14, 18][size_format]
        regenerated = (bits >> 4) & ((1 << width) - 1)
        payload_size = bits >> (width + 4)
    offset = header_size + payload_size
    first = block[offset]
    if first < 128:
        sequences, count_size = first, 1
    elif first < 255:
        sequences, count_size = ((first - 128) << 8) + block[offset + 1], 2
    else:
        sequences, count_size = int.from_bytes(block[offset + 1:offset + 3], "little") + 0x7f00, 3
    result.update(literals_type=literal_kind, literals_size=regenerated, sequences=sequences)
    if sequences:
        modes = block[offset + count_size]
        result["sequence_modes"] = {"literal_length": modes >> 6,
                                    "offset": (modes >> 4) & 3, "match_length": (modes >> 2) & 3}
    return result


def prepared_vectors(temporary, outputs, records):
    executable = temporary / "dictionary-vectors"
    generated = temporary / "prepared"
    generated.mkdir()
    samples = training_samples()
    (generated / "training-samples.bin").write_bytes(samples)
    (generated / "boundary-dictionary.raw").write_bytes(outputs["boundary-dictionary.raw"])
    metadata = json.loads(subprocess.check_output([str(executable), "generate", str(generated)], text=True))
    records.append({"path": "training-samples.bin", "role": "training_samples", "sample_sizes": [512] * 256})
    for dictionary in metadata["dictionaries"]:
        records.append({"path": dictionary["path"], "role": "formatted_dictionary"})
    results = []
    for case in metadata["cases"]:
        name = case["name"]
        frame_path = name + ".zst"
        expected_path = name + ".expected"
        frame = (generated / frame_path).read_bytes()
        case["first_block"] = first_block_modes(frame)
        records += [{"path": frame_path, "role": "dictionary_frame", "dictionary": case["dictionary"],
                     "expected_output": expected_path, "rust_expectation": "accept_with_persistent_dictionary"},
                    {"path": expected_path, "role": "original_frame_input"}]
        methods = [("direct", 0), ("prepared", 0), ("stream", 1), ("stream", 7), ("stream", 1024)]
        if case["prefix_compression"]:
            methods += [("prefix-stream", 7)]
        for method, chunk in methods:
            arguments = [str(executable), "probe", str(generated / case["dictionary"]),
                         str(generated / frame_path), str(generated / expected_path),
                         case["mode"], method, str(chunk)]
            row = json.loads(subprocess.check_output(arguments, text=True))
            row.update(case=name, assertions=False, method=method, output_chunk=chunk)
            row["outcome"] = "accepted" if row["success"] else "rejected"
            expected_success = not (name == "prefix-reused-concatenation" and method == "prefix-stream")
            assert row["success"] == expected_success, row
            if row["success"]:
                assert row["matches_expected"], row
            results.append(row)
    cases = {case["name"]: case for case in metadata["cases"]}
    for name in ["dictionary-treeless-literals", "dictionary-treeless-four-streams"]:
        assert cases[name]["first_block"]["literals_type"] == 3
    assert set(cases["dictionary-repeat-sequences"]["first_block"]["sequence_modes"].values()) == {3}
    for name in ["raw-small-window", "formatted-omitted-id", "prefix-first"]:
        assert not cases[name]["without_dictionary_success"], name
    assert cases["prefix-second"]["without_dictionary_success"]
    for path in sorted(generated.iterdir()):
        outputs[path.name] = path.read_bytes()
    return metadata, results


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
        prepared, prepared_results = prepared_vectors(Path(directory), outputs, records)
        results += prepared_results
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
        "eviction_evidence": eviction, "prepared_vectors": prepared,
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
