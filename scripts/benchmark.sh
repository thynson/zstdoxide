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

for configuration in safe unsafe-byte-cursor unsafe-bitstream unsafe-fse unsafe-huffman unsafe-history unsafe-ring-history unsafe-match-table all; do
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
}

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
            }
    expected_cases = {"byte_cursor", "reverse_bits", "fse_lookup", "huffman_decode",
                      "history_overlap", "ring_overlap", "match_table", "compress",
                      "decompress", "stream_compress", "stream_decompress"}
    if rows.keys() != expected_cases:
        raise SystemExit("Expected all eleven benchmark cases in " + name)
    required = ("seed", "samples", "sample_ms", "compress_original_bytes",
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
                "stream_decompress_single_frame_long_retained_bytes")
    if any(key not in metadata for key in required):
        raise SystemExit("Missing workload metadata in " + name)
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

lines = [
    "# Backend measurements",
    "",
    "All eleven cases produced identical deterministic fingerprints in every build.",
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
    "",
    "Raw sample ranges and fingerprints are recorded in the adjacent TSV files.",
    "Compiler, machine, and timestamp are recorded in `environment.txt`.",
    "The benchmark prints its seed, sample count, and duration in each TSV file.",
    "",
])
(root / "summary.md").write_text("\n".join(lines))
print("Verified backend fingerprints, workload sizes, and full-output checksums. Summary: " + str(root / "summary.md"))
PY
