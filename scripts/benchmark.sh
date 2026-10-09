#!/usr/bin/env bash
# Compare identical deterministic workloads across all supported backend choices.
set -euo pipefail
cd -- "$(dirname -- "$0")/.."

output_dir="${1:-target/backend-benchmarks}"
toolchain="${ZSTDOXIDE_TOOLCHAIN:-1.89.0}"
mkdir -p "$output_dir"
source_fingerprint() {
    python3 - <<'HASH'
import hashlib
import pathlib
root = pathlib.Path(".")
files = [*root.glob("src/**/*.rs"), *root.glob("benches/**/*.rs"),
         *root.glob("tests/fixtures/*.zst"), pathlib.Path("Cargo.toml"),
         pathlib.Path("scripts/benchmark.sh")]
files.extend(pathlib.Path(name) for name in (
    "tests/upstream/dictionaries/formatted.dict",
    "tests/upstream/dictionaries/training-samples.bin",
    "tests/upstream/data/golden-dictionaries/http-dict-missing-symbols",
    "tests/upstream/data/generated/http-dictionary.zst",
    "tests/upstream/data/golden-compression/http",
))
if pathlib.Path("Cargo.lock").exists():
    files.append(pathlib.Path("Cargo.lock"))
digest = hashlib.sha256()
for file in sorted(files):
    digest.update(str(file).encode() + b"\0" + file.read_bytes() + b"\0")
print(digest.hexdigest())
HASH
}
code_fingerprint="$(source_fingerprint)"
printf '%s\n' "$code_fingerprint" > "$output_dir/source.sha256"
{
    date -u '+Timestamp: %Y-%m-%d %H:%M:%S UTC'
    rustc "+$toolchain" -vV
    uname -a
    git rev-parse HEAD
    git status --short
    if command -v sysctl >/dev/null 2>&1; then
        sysctl -n machdep.cpu.brand_string 2>/dev/null || printf 'CPU model unavailable under current permissions.\n'
    elif command -v lscpu >/dev/null 2>&1; then
        lscpu
    fi
} > "$output_dir/environment.txt"

for configuration in safe unsafe-byte-cursor unsafe-bitstream unsafe-fse unsafe-huffman unsafe-history unsafe-ring-history unsafe-match-table unsafe-dictionary unsafe-training unsafe-fse-encoder unsafe-huffman-codebook unsafe-match-chain all; do
    printf 'Benchmarking %s\n' "$configuration"
    case "$configuration" in
        safe)
            cargo "+$toolchain" bench --bench backends | tee "$output_dir/$configuration.tsv"
            ;;
        all)
            cargo "+$toolchain" bench --bench backends --all-features | tee "$output_dir/$configuration.tsv"
            ;;
        *)
            cargo "+$toolchain" bench --bench backends --features "$configuration" | tee "$output_dir/$configuration.tsv"
            ;;
    esac
    if [[ "$(source_fingerprint)" != "$code_fingerprint" ]]; then
        printf 'Source changed during the benchmark; restart all configurations.\n' >&2
        exit 1
    fi
done

python3 - "$output_dir" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
configurations = {
    "unsafe-byte-cursor": "byte_cursor",
    "unsafe-bitstream": "reverse_bits",
    "unsafe-fse": "fse_lookup",
    "unsafe-huffman": "huffman_decode",
    "unsafe-history": "history_overlap",
    "unsafe-ring-history": "ring_overlap",
    "unsafe-match-table": "match_table",
    "unsafe-dictionary": "dictionary_content",
    "unsafe-training": "raw_training",
    "unsafe-fse-encoder": "fse_encode_lookup",
    "unsafe-huffman-codebook": "huffman_codebook",
    "unsafe-match-chain": "match_chain",
}

strategies = ("greedy", "chain", "lazy")
strategy_sources = ("compressible", "ambiguous")
strategy_cases = tuple(f"strategy_{strategy}_{source}"
                       for strategy in strategies for source in strategy_sources)

def read(name):
    rows = {}
    metadata = {}
    for line in (root / (name + ".tsv")).read_text().splitlines():
        fields = line.split("\t")
        if fields[0] == "META":
            _, key, value = fields
            if key in metadata:
                raise SystemExit("Duplicate benchmark metadata: " + key)
            metadata[key] = value
        elif fields[0] == "RESULT":
            _, case, unit, count, median, minimum, maximum, fingerprint = fields
            if case in rows:
                raise SystemExit("Duplicate benchmark case: " + case)
            rows[case] = {
                "unit": unit,
                "count": count,
                "median": float(median),
                "minimum": float(minimum),
                "maximum": float(maximum),
                "fingerprint": fingerprint,
                "kind": "timed",
            }
        elif fields[0] == "CHECK":
            _, case, unit, count, fingerprint = fields
            if case in rows:
                raise SystemExit("Duplicate smoke case: " + case)
            rows[case] = {"unit": unit, "count": count, "fingerprint": fingerprint, "kind": "smoke"}
    expected_cases = {"byte_cursor", "reverse_bits", "fse_lookup", "huffman_decode",
                      "history_overlap", "ring_overlap", "match_table", "compress",
                      "decompress", "stream_compress", "stream_decompress",
                      "dictionary_content", "dictionary_compress", "dictionary_decompress",
                      "raw_training", "fse_encode_lookup", "huffman_codebook", "match_chain",
                      *strategy_cases}
    if rows.keys() != expected_cases:
        raise SystemExit("Expected all 24 benchmark cases in " + name)
    required = ("mode", "features", "seed", "samples", "sample_ms", "compress_original_bytes",
                "compress_encoded_bytes", "compress_output_fnv1a64",
                "decompress_encoded_bytes", "decompress_decoded_bytes",
                "decompress_output_fnv1a64", "ring_output_fnv1a64", "ring_window_bytes",
                "stream_compress_original_bytes", "stream_compress_encoded_bytes",
                "stream_compress_output_fnv1a64", "stream_decompress_encoded_bytes",
                "stream_decompress_decoded_bytes", "stream_decompress_output_fnv1a64",
                "stream_compress_reset_retained_bytes", "stream_decompress_reset_retained_bytes",
                "stream_compress_peak_retained_bytes_4_blocks",
                "stream_compress_peak_retained_bytes_64_blocks",
                "stream_decompress_peak_retained_bytes_4_frames",
                "stream_decompress_peak_retained_bytes_64_frames",
                "stream_decompress_single_frame_short_bytes",
                "stream_decompress_single_frame_long_bytes",
                "stream_decompress_single_frame_short_retained_bytes",
                "stream_decompress_single_frame_long_retained_bytes",
                "dictionary_serialized_bytes", "dictionary_content_bytes", "dictionary_id",
                "dictionary_fnv1a64", "dictionary_content_read_bytes", "dictionary_content_output_fnv1a64",
                "dictionary_compress_original_bytes", "dictionary_compress_original_fnv1a64",
                "dictionary_compress_encoded_bytes", "dictionary_compress_without_dictionary_bytes",
                "dictionary_compress_output_fnv1a64", "dictionary_compress_reset_retained_bytes",
                "dictionary_decompress_dictionary_bytes", "dictionary_decompress_dictionary_content_bytes",
                "dictionary_decompress_dictionary_id", "dictionary_decompress_dictionary_fnv1a64",
                "dictionary_decompress_frames", "dictionary_decompress_encoded_bytes",
                "dictionary_decompress_decoded_bytes", "dictionary_decompress_output_fnv1a64",
                "dictionary_decompress_peak_retained_bytes", "dictionary_decompress_reset_retained_bytes",
                "raw_training_sample_count", "raw_training_input_bytes", "raw_training_input_fnv1a64",
                "raw_training_sample_bytes_limit", "raw_training_dictionary_bytes_limit",
                "raw_training_segment_bytes", "raw_training_output_bytes", "raw_training_output_fnv1a64")
    required += ("fse_encode_lookup_transitions", "fse_encode_lookup_table_log",
                 "fse_encode_lookup_retained_bytes", "fse_encode_lookup_output_fnv1a64",
                 "huffman_codebook_symbols", "huffman_codebook_table_log",
                 "huffman_codebook_inline_bytes", "huffman_codebook_output_fnv1a64",
                 "match_chain_queries", "match_chain_candidates", "match_chain_node_limit",
                 "match_chain_visit_budget", "match_chain_retained_bytes", "match_chain_output_fnv1a64")
    required += tuple(f"{case}_{field}" for case in strategy_cases for field in (
        "original_bytes", "original_fnv1a64", "encoded_bytes", "output_fnv1a64",
        "reset_retained_bytes", "dictionary_bytes", "dictionary_fnv1a64"))
    if any(key not in metadata for key in required):
        raise SystemExit("Missing workload metadata in " + name)
    for source in strategy_sources:
        for field in ("original_bytes", "original_fnv1a64", "dictionary_bytes", "dictionary_fnv1a64"):
            if len({metadata[f"strategy_{strategy}_{source}_{field}"] for strategy in strategies}) != 1:
                raise SystemExit(f"Strategy inputs differ in {name}: {source} / {field}")
    if metadata["mode"] not in {"smoke", "timed"} or any(row["kind"] != metadata["mode"] for row in rows.values()):
        raise SystemExit("Mixed or invalid measurement modes in " + name)
    expected_features = {"safe"} if name == "safe" else set(configurations) if name == "all" else {name}
    if set(metadata["features"].split(",")) != expected_features:
        raise SystemExit("Enabled features differ from requested configuration: " + name)
    return rows, {key: value for key, value in metadata.items() if key != "features"}

safe, reference_metadata = read("safe")
parsed = {name: read(name) for name in [*configurations, "all"]}
results = {name: cases for name, (cases, _) in parsed.items()}
for name, (cases, metadata) in parsed.items():
    if metadata != reference_metadata:
        raise SystemExit("Workload sizes, settings, or full-output checksums differ: " + name)
    if cases.keys() != safe.keys():
        raise SystemExit("Benchmark cases differ for " + name)
    for case, row in cases.items():
        for key in ("unit", "count", "fingerprint"):
            if row[key] != safe[case][key]:
                raise SystemExit(f"Mismatch: {name} / {case} / {key}")

if reference_metadata["mode"] == "smoke":
    lines = ["# Backend smoke checks", "",
             "All 24 deterministic operations and complete workload metadata agree across",
             "the safe build, twelve independent unsafe-feature builds and the combined build.",
             "No timing clocks were started and no throughput comparisons were collected.", "",
             "| Case | Unit | Units per operation | Fingerprint |", "| --- | --- | ---: | --- |"]
    for case, row in safe.items():
        lines.append(f"| {case} | {row['unit']} | {row['count']} | `{row['fingerprint']}` |")
    lines.extend(["", "Full dictionary/output hashes, resource caps and retained-context capacities",
                  "are recorded in the adjacent TSV files; source/input identity is in source.sha256.", ""])
    (root / "summary.md").write_text("\n".join(lines))
    print("Verified backend smoke fingerprints and all metadata without collecting timings. Summary: " + str(root / "summary.md"))
    sys.exit(0)

lines = [
    "# Backend measurements",
    "",
    "All 24 cases produced identical deterministic fingerprints in every build.",
    "Whole-codec output sizes and full-output FNV-1a checksums also match across builds.",
    "These noncryptographic checksums supplement full-output interoperability tests.",
    "Each table entry is the median of the configured timed samples. Higher relative",
    "throughput is better; 1.00 means equal to the safe build. Workstation noise and",
    "allocation/sampled-fingerprint overhead are included; these are not universal speedups.",
    "Full-output checksums and the following sizes are measured outside the timed regions.",
    "Streaming timings reuse contexts and caller buffers, including reset and progress checks;",
    "one-shot timings include allocation. Their difference is not a backend-only speedup.",
    "",
    "| Whole-codec workload | Original/decoded bytes | Compressed bytes | Compressed/original | Output FNV-1a |",
    "| --- | ---: | ---: | ---: | --- |",
    f"| Compression input | {reference_metadata['compress_original_bytes']} | {reference_metadata['compress_encoded_bytes']} | {int(reference_metadata['compress_encoded_bytes']) / int(reference_metadata['compress_original_bytes']):.4%} | `{reference_metadata['compress_output_fnv1a64']}` |",
    f"| Official text decompression fixture | {reference_metadata['decompress_decoded_bytes']} | {reference_metadata['decompress_encoded_bytes']} | {int(reference_metadata['decompress_encoded_bytes']) / int(reference_metadata['decompress_decoded_bytes']):.4%} | `{reference_metadata['decompress_output_fnv1a64']}` |",
    f"| Streaming compression input | {reference_metadata['stream_compress_original_bytes']} | {reference_metadata['stream_compress_encoded_bytes']} | {int(reference_metadata['stream_compress_encoded_bytes']) / int(reference_metadata['stream_compress_original_bytes']):.4%} | `{reference_metadata['stream_compress_output_fnv1a64']}` |",
    f"| Streaming official text fixture | {reference_metadata['stream_decompress_decoded_bytes']} | {reference_metadata['stream_decompress_encoded_bytes']} | {int(reference_metadata['stream_decompress_encoded_bytes']) / int(reference_metadata['stream_decompress_decoded_bytes']):.4%} | `{reference_metadata['stream_decompress_output_fnv1a64']}` |",
    f"| Formatted-dictionary compression | {reference_metadata['dictionary_compress_original_bytes']} | {reference_metadata['dictionary_compress_encoded_bytes']} | {int(reference_metadata['dictionary_compress_encoded_bytes']) / int(reference_metadata['dictionary_compress_original_bytes']):.4%} | `{reference_metadata['dictionary_compress_output_fnv1a64']}` |",
    f"| Prepared HTTP dictionary decoding | {reference_metadata['dictionary_decompress_decoded_bytes']} | {reference_metadata['dictionary_decompress_encoded_bytes']} | {int(reference_metadata['dictionary_decompress_encoded_bytes']) / int(reference_metadata['dictionary_decompress_decoded_bytes']):.4%} | `{reference_metadata['dictionary_decompress_output_fnv1a64']}` |",
    "",
    "| Feature | Target workload | Safe ns/unit | Feature ns/unit | Relative throughput |",
    "| --- | --- | ---: | ---: | ---: |",
]
for name, case in configurations.items():
    baseline = safe[case]["median"]
    optimized = results[name][case]["median"]
    lines.append(f"| `{name}` | {case} | {baseline:.4f} | {optimized:.4f} | {baseline / optimized:.3f}x |")
lines.extend([
    "",
    "| Workload | Safe ns/unit | All features ns/unit | Relative throughput |",
    "| --- | ---: | ---: | ---: |",
])
for case, baseline in safe.items():
    optimized = results["all"][case]["median"]
    lines.append(f"| {case} | {baseline['median']:.4f} | {optimized:.4f} | {baseline['median'] / optimized:.3f}x |")
lines.extend([
    "",
    "The named strategies receive identical source bytes within each source group.",
    "The ambiguous source uses one identical prepared raw dictionary; preparation is",
    "outside timing, while reset, reused context work, and sampled hashes are included.",
    "The compressible source has no dictionary. Strategies may produce different frames;",
    "each strategy's frame bytes, sizes, and context capacities match across all backends.",
    "These independent Rust policies do not represent upstream compression levels.",
    "",
    "| Strategy | Source | Original bytes | Dictionary bytes | Compressed bytes | Compressed/original | Reset retained bytes | Output FNV-1a |",
    "| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |",
])
for strategy in strategies:
    for source in strategy_sources:
        key = f"strategy_{strategy}_{source}"
        original = int(reference_metadata[f"{key}_original_bytes"])
        encoded = int(reference_metadata[f"{key}_encoded_bytes"])
        lines.append(f"| {strategy} | {source} | {original} | {reference_metadata[f'{key}_dictionary_bytes']} | {encoded} | {encoded / original:.4%} | {reference_metadata[f'{key}_reset_retained_bytes']} | `{reference_metadata[f'{key}_output_fnv1a64']}` |")
lines.extend([
    "",
    "Inverse-FSE, Huffman-codebook and chain-candidate setup is outside timing.",
    "Complete inverse relations, canonical ranks and candidate coordinates are checked",
    "before timing; all result records have full byte hashes in the adjacent TSV files.",
    "The chain lookup workload has sixteen candidates per key and a sixteen-visit cap;",
    "its retained-capacity field counts only owned chain storage, excluding input and result vectors.",
    "",
    "Retained memory is the maximum context-reported heap capacity observed after process",
    "calls. It excludes inline state, caller buffers, allocator overhead, and transient scratch.",
    "These checks run outside timed regions and verify exact output bytes. Equal capacity",
    "for the longer run establishes a plateau for this workload, not a universal memory limit.",
    "",
    "| Context | Short workload | Long workload | Short retained bytes | Long retained bytes |",
    "| --- | --- | --- | ---: | ---: |",
    f"| Decoder | 4 concatenated frames | 64 concatenated frames | {reference_metadata['stream_decompress_peak_retained_bytes_4_frames']} | {reference_metadata['stream_decompress_peak_retained_bytes_64_frames']} |",
    f"| Decoder, one frame | {reference_metadata['stream_decompress_single_frame_short_bytes']} decoded bytes | {reference_metadata['stream_decompress_single_frame_long_bytes']} decoded bytes | {reference_metadata['stream_decompress_single_frame_short_retained_bytes']} | {reference_metadata['stream_decompress_single_frame_long_retained_bytes']} |",
    f"| Encoder | 4 input blocks | 64 input blocks, one frame | {reference_metadata['stream_compress_peak_retained_bytes_4_blocks']} | {reference_metadata['stream_compress_peak_retained_bytes_64_blocks']} |",
    "",
    f"The decoder delivered {reference_metadata['stream_decompress_memory_output_bytes']} bytes in the long memory trial.",
    f"The encoder accepted {reference_metadata['stream_compress_memory_input_bytes']} bytes in {reference_metadata['stream_compress_memory_block_bytes']}-byte blocks.",
    f"Streaming input/output chunks are {reference_metadata['stream_decode_input_chunk_bytes']}/{reference_metadata['stream_decode_output_chunk_bytes']} bytes for decoding and {reference_metadata['stream_encode_input_chunk_bytes']}/{reference_metadata['stream_encode_output_chunk_bytes']} bytes for encoding.",
    f"After reset, the timed decoder retains {reference_metadata['stream_decompress_reset_retained_bytes']} bytes and the timed encoder {reference_metadata['stream_compress_reset_retained_bytes']} bytes.",
    f"With persistent prepared dictionaries, decoder/encoder reset retains {reference_metadata['dictionary_decompress_reset_retained_bytes']}/{reference_metadata['dictionary_compress_reset_retained_bytes']} bytes respectively, including each selected dictionary once per context report.",
    f"Raw training admits at most {reference_metadata['raw_training_sample_bytes_limit']} bytes from {reference_metadata['raw_training_sample_count']} samples ({reference_metadata['raw_training_input_bytes']} input bytes), with {reference_metadata['raw_training_segment_bytes']}-byte segments and a {reference_metadata['raw_training_dictionary_bytes_limit']}-byte output cap.",
    f"The selected raw dictionary contains {reference_metadata['raw_training_output_bytes']} bytes with full-output FNV-1a `{reference_metadata['raw_training_output_fnv1a64']}`. Training timings include scratch allocation and the complete selection algorithm.",
    "",
    "Raw sample ranges and fingerprints are recorded in the adjacent TSV files.",
    "Compiler, machine, and timestamp are recorded in `environment.txt`.",
    "The benchmark prints its seed, sample count, and duration in each TSV file.",
    "",
])
(root / "summary.md").write_text("\n".join(lines))
print("Verified backend fingerprints, workload sizes, and full-output checksums. Summary: " + str(root / "summary.md"))
PY
