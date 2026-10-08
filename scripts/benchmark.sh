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
         *root.glob("tests/fixtures/*.zst"), pathlib.Path("Cargo.toml")]
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
        sysctl -n machdep.cpu.brand_string 2>/dev/null || true
    elif command -v lscpu >/dev/null 2>&1; then
        lscpu
    fi
} > "$output_dir/environment.txt"

for configuration in safe unsafe-byte-cursor unsafe-bitstream unsafe-fse unsafe-huffman unsafe-history unsafe-match-table all; do
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
    "unsafe-match-table": "match_table",
}

def read(name):
    rows = {}
    for line in (root / (name + ".tsv")).read_text().splitlines():
        fields = line.split("\t")
        if fields[0] == "RESULT":
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
    if len(rows) != 8:
        raise SystemExit("Expected eight benchmark cases in " + name)
    return rows

safe = read("safe")
results = {name: read(name) for name in [*configurations, "all"]}
for name, cases in results.items():
    if cases.keys() != safe.keys():
        raise SystemExit("Benchmark cases differ for " + name)
    for case, row in cases.items():
        for key in ("unit", "count", "fingerprint"):
            if row[key] != safe[case][key]:
                raise SystemExit(f"Mismatch: {name} / {case} / {key}")

lines = [
    "# Backend measurements",
    "",
    "All eight cases produced identical deterministic fingerprints in every build.",
    "Fingerprints sample byte output; interoperability tests provide full-output checks.",
    "Each table entry is the median of the configured timed samples. Higher relative",
    "throughput is better; 1.00 means equal to the safe build. Workstation noise and",
    "allocation/checksum overhead are included; these are not universal speedups.",
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
    "Raw sample ranges and fingerprints are recorded in the adjacent TSV files.",
    "Compiler, machine, and timestamp are recorded in `environment.txt`.",
    "The benchmark prints its seed, sample count, and duration in each TSV file.",
    "",
])
(root / "summary.md").write_text("\n".join(lines))
print("Verified every backend fingerprint. Summary: " + str(root / "summary.md"))
PY
